use crate::database;
use crate::models::course::{Course, CreateCourseInput, UpdateCourseInput};
use tauri::AppHandle;

#[tauri::command]
pub fn create_course(
    app_handle: AppHandle,
    input: CreateCourseInput,
) -> Result<Course, String> {
    let conn = database::get_connection(&app_handle).map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT INTO courses (name, description) VALUES (?1, ?2)",
        [&input.name, &input.description],
    )
    .map_err(|e| e.to_string())?;

    let id = conn.last_insert_rowid();

    let course = conn
        .query_row(
            "SELECT id, name, description, created_at, updated_at FROM courses WHERE id = ?1",
            [id],
            |row| {
                Ok(Course {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            },
        )
        .map_err(|e| e.to_string())?;

    Ok(course)
}

#[tauri::command]
pub fn get_courses(app_handle: AppHandle) -> Result<Vec<Course>, String> {
    let conn = database::get_connection(&app_handle).map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare("SELECT id, name, description, created_at, updated_at FROM courses ORDER BY updated_at DESC")
        .map_err(|e| e.to_string())?;

    let courses = stmt
        .query_map([], |row| {
            Ok(Course {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(courses)
}

#[tauri::command]
pub fn update_course(
    app_handle: AppHandle,
    input: UpdateCourseInput,
) -> Result<Course, String> {
    let conn = database::get_connection(&app_handle).map_err(|e| e.to_string())?;

    let mut updates = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(name) = &input.name {
        updates.push("name = ?");
        params.push(Box::new(name.clone()));
    }

    if let Some(description) = &input.description {
        updates.push("description = ?");
        params.push(Box::new(description.clone()));
    }

    if updates.is_empty() {
        return Err("No fields to update".to_string());
    }

    updates.push("updated_at = datetime('now', 'localtime')");

    let sql = format!(
        "UPDATE courses SET {} WHERE id = ?",
        updates.join(", ")
    );

    params.push(Box::new(input.id));

    let params_ref: Vec<&dyn rusqlite::ToSql> = params.iter().map(|b| &**b as &dyn rusqlite::ToSql).collect();

    conn.execute(&sql, params_ref.as_slice())
        .map_err(|e| e.to_string())?;

    let course = conn
        .query_row(
            "SELECT id, name, description, created_at, updated_at FROM courses WHERE id = ?1",
            [input.id],
            |row| {
                Ok(Course {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            },
        )
        .map_err(|e| e.to_string())?;

    Ok(course)
}

#[tauri::command]
pub fn delete_course(app_handle: AppHandle, id: i64) -> Result<(), String> {
    let conn = database::get_connection(&app_handle).map_err(|e| e.to_string())?;

    let rows_affected = conn
        .execute("DELETE FROM courses WHERE id = ?1", [id])
        .map_err(|e| e.to_string())?;

    if rows_affected == 0 {
        return Err("Course not found".to_string());
    }

    Ok(())
}
