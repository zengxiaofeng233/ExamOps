use crate::database;
use crate::error::{AppError, AppResult, OptionalRow};
use crate::models::course::{Course, CreateCourseInput, UpdateCourseInput};
use rusqlite::Connection;
use tauri::AppHandle;

const COURSE_COLUMNS: &str = "id, name, description, exam_date, created_at, updated_at";

fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Course> {
    Ok(Course {
        id: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        exam_date: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

pub fn find(conn: &Connection, id: i64) -> AppResult<Course> {
    conn.query_row(
        &format!("SELECT {COURSE_COLUMNS} FROM courses WHERE id = ?1"),
        [id],
        from_row,
    )
    .or_not_found("课程")
}

fn clean_name(name: &str) -> AppResult<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::invalid("课程名称不能为空"));
    }
    if name.chars().count() > 100 {
        return Err(AppError::invalid("课程名称不能超过 100 个字符"));
    }
    Ok(name.to_string())
}

/// 新建课程。
#[tauri::command]
pub fn create_course(app_handle: AppHandle, input: CreateCourseInput) -> AppResult<Course> {
    let conn = database::get_connection(&app_handle)?;
    let name = clean_name(&input.name)?;
    conn.execute(
        "INSERT INTO courses (name, description) VALUES (?1, ?2)",
        [&name, input.description.trim()],
    )?;
    find(&conn, conn.last_insert_rowid())
}

/// 全部课程。
#[tauri::command]
pub fn get_courses(app_handle: AppHandle) -> AppResult<Vec<Course>> {
    let conn = database::get_connection(&app_handle)?;
    let mut stmt = conn.prepare(&format!(
        "SELECT {COURSE_COLUMNS} FROM courses ORDER BY updated_at DESC, id DESC"
    ))?;
    let courses = stmt
        .query_map([], from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(courses)
}

/// 单个课程。
#[tauri::command]
pub fn get_course(app_handle: AppHandle, id: i64) -> AppResult<Course> {
    let conn = database::get_connection(&app_handle)?;
    find(&conn, id)
}

/// 修改名称、简介、考试日期（考试日期传空字符串表示清除）。
#[tauri::command]
pub fn update_course(app_handle: AppHandle, input: UpdateCourseInput) -> AppResult<Course> {
    let conn = database::get_connection(&app_handle)?;
    find(&conn, input.id)?;
    if let Some(name) = &input.name {
        conn.execute(
            "UPDATE courses SET name = ?1, updated_at = datetime('now', 'localtime') WHERE id = ?2",
            rusqlite::params![clean_name(name)?, input.id],
        )?;
    }
    if let Some(description) = &input.description {
        conn.execute(
            "UPDATE courses SET description = ?1, updated_at = datetime('now', 'localtime') WHERE id = ?2",
            rusqlite::params![description.trim(), input.id],
        )?;
    }
    if let Some(date) = &input.exam_date {
        let date = date.trim();
        let value = if date.is_empty() {
            None
        } else {
            chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").map_err(|_| AppError::invalid("考试日期格式应为 YYYY-MM-DD"))?;
            Some(date.to_string())
        };
        conn.execute("UPDATE courses SET exam_date = ?1 WHERE id = ?2", rusqlite::params![value, input.id])?;
    }
    find(&conn, input.id)
}

/// 删除课程及其全部数据和资料副本。
#[tauri::command]
pub async fn delete_course(
    paths: tauri::State<'_, crate::database::AppPaths>,
    tasks: tauri::State<'_, crate::documents::DocumentTasks>,
    id: i64,
) -> AppResult<()> {
    let _guard = tasks.0.lock().await;
    let _registration = tasks.2.lock().await;
    let paths = paths.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut conn = paths.connect()?;
        crate::documents::delete_course(&mut conn, &paths, id)
    })
    .await?
}
