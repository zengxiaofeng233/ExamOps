//! Stage 4 commands: knowledge points, learning paths, course videos, notes
//! and the study state.

use crate::ai::AiState;
use crate::database::{self, AppPaths};
use crate::error::{AppError, AppResult};
use crate::knowledge::{self, ExtractResult, KnowledgeInput, KnowledgePoint};
use crate::notes::{self, Note, NoteInput};
use crate::paths::{self, LearningPath, PathDetail, PathInput, PathStep, RecommendInput, StepInput, TemplateInput};
use crate::resources::{self, MarkInput, Resource, ResourceInput};
use crate::study::{self, CourseDigest, StudyState};
use tauri::{AppHandle, State};

// --------------------------------------------------------------------------- knowledge points

/// 课程的知识点（含掌握度与下次复习日期）。
#[tauri::command]
pub fn list_knowledge_points(app_handle: AppHandle, course_id: i64) -> AppResult<Vec<KnowledgePoint>> {
    let conn = database::get_connection(&app_handle)?;
    knowledge::list(&conn, course_id)
}

/// 新建或修改知识点。
#[tauri::command]
pub fn save_knowledge_point(app_handle: AppHandle, input: KnowledgeInput) -> AppResult<KnowledgePoint> {
    let conn = database::get_connection(&app_handle)?;
    knowledge::save(&conn, input)
}

/// 删除知识点。
#[tauri::command]
pub fn delete_knowledge_point(app_handle: AppHandle, id: i64) -> AppResult<()> {
    let conn = database::get_connection(&app_handle)?;
    knowledge::delete(&conn, id)
}

/// 标记或取消“疑难”。
#[tauri::command]
pub fn set_knowledge_point_difficult(app_handle: AppHandle, id: i64, difficult: bool) -> AppResult<KnowledgePoint> {
    let conn = database::get_connection(&app_handle)?;
    knowledge::set_difficult(&conn, id, difficult)
}

/// 设为“学过”或“未学”；“已掌握”只能由测验结果授予。
#[tauri::command]
pub fn set_knowledge_point_status(app_handle: AppHandle, id: i64, status: String) -> AppResult<KnowledgePoint> {
    let conn = database::get_connection(&app_handle)?;
    match status.as_str() {
        "learned" => {
            knowledge::mark_learned(&conn, &[id])?;
            knowledge::get(&conn, id)
        }
        "new" => knowledge::reset_status(&conn, id),
        _ => Err(AppError::invalid("“已掌握”只能通过测验获得")),
    }
}

/// 从章节标题生成知识点（不需要 AI），返回新增数量。
#[tauri::command]
pub fn generate_knowledge_points(app_handle: AppHandle, course_id: i64, document_id: Option<i64>) -> AppResult<usize> {
    let conn = database::get_connection(&app_handle)?;
    knowledge::from_headings(&conn, course_id, document_id)
}

/// AI 从某一章提取知识点（结果校验并去重）。
#[tauri::command]
pub async fn extract_knowledge_points(paths: State<'_, AppPaths>, ai: State<'_, AiState>, course_id: i64, section_id: i64) -> AppResult<ExtractResult> {
    knowledge::extract_with_ai(paths.inner(), ai.inner(), course_id, section_id).await
}

// --------------------------------------------------------------------------- learning paths

/// 课程的学习路径。
#[tauri::command]
pub fn list_learning_paths(app_handle: AppHandle, course_id: i64) -> AppResult<Vec<LearningPath>> {
    let conn = database::get_connection(&app_handle)?;
    paths::list(&conn, course_id)
}

/// 学习路径及其全部步骤。
#[tauri::command]
pub fn get_learning_path(app_handle: AppHandle, id: i64) -> AppResult<PathDetail> {
    let conn = database::get_connection(&app_handle)?;
    paths::detail(&conn, id)
}

/// 新建或修改学习路径（名称、说明、模式、是否归档）。
#[tauri::command]
pub fn save_learning_path(app_handle: AppHandle, input: PathInput) -> AppResult<LearningPath> {
    let conn = database::get_connection(&app_handle)?;
    paths::save_path(&conn, input)
}

/// 删除学习路径。
#[tauri::command]
pub fn delete_learning_path(app_handle: AppHandle, id: i64) -> AppResult<()> {
    let conn = database::get_connection(&app_handle)?;
    paths::delete_path(&conn, id)
}

/// 设为当前学习路径。
#[tauri::command]
pub fn activate_learning_path(app_handle: AppHandle, course_id: i64, id: i64) -> AppResult<StudyState> {
    let conn = database::get_connection(&app_handle)?;
    if paths::get_path(&conn, id)?.course_id != course_id {
        return Err(AppError::invalid("学习路径不属于该课程"));
    }
    study::set_active_path(&conn, course_id, Some(id))?;
    study::state(&conn, course_id)
}

/// 按章节模板生成学习路径。
#[tauri::command]
pub fn create_path_from_template(app_handle: AppHandle, input: TemplateInput) -> AppResult<PathDetail> {
    let mut conn = database::get_connection(&app_handle)?;
    paths::from_template(&mut conn, input)
}

/// AI 推荐学习路径（每一步都校验能否对应到课程资料）。
#[tauri::command]
pub async fn recommend_learning_path(paths: State<'_, AppPaths>, ai: State<'_, AiState>, input: RecommendInput) -> AppResult<PathDetail> {
    paths::recommend_with_ai(paths.inner(), ai.inner(), input).await
}

/// 新建步骤（可插入到指定步骤之后）或修改步骤。
#[tauri::command]
pub fn save_path_step(app_handle: AppHandle, input: StepInput) -> AppResult<PathStep> {
    let mut conn = database::get_connection(&app_handle)?;
    paths::save_step(&mut conn, input)
}

/// 删除步骤。
#[tauri::command]
pub fn delete_path_step(app_handle: AppHandle, id: i64) -> AppResult<()> {
    let mut conn = database::get_connection(&app_handle)?;
    paths::delete_step(&mut conn, id)
}

/// 调整步骤顺序（必须包含路径的全部步骤）。
#[tauri::command]
pub fn reorder_path_steps(app_handle: AppHandle, path_id: i64, step_ids: Vec<i64>) -> AppResult<Vec<PathStep>> {
    let mut conn = database::get_connection(&app_handle)?;
    paths::reorder(&mut conn, path_id, &step_ids)
}

/// 设置步骤状态：todo / in_progress / done / skipped。
#[tauri::command]
pub fn set_path_step_status(app_handle: AppHandle, id: i64, status: String, note: Option<String>) -> AppResult<PathStep> {
    let conn = database::get_connection(&app_handle)?;
    paths::set_status(&conn, id, &status, note.as_deref())
}

// --------------------------------------------------------------------------- course videos

/// 课程的网课与链接（含时间戳）。
#[tauri::command]
pub fn list_resources(app_handle: AppHandle, course_id: i64) -> AppResult<Vec<Resource>> {
    let conn = database::get_connection(&app_handle)?;
    resources::list(&conn, course_id)
}

/// 新建或修改网课（只允许 http/https 链接）。
#[tauri::command]
pub fn save_resource(app_handle: AppHandle, input: ResourceInput) -> AppResult<Resource> {
    let conn = database::get_connection(&app_handle)?;
    resources::save(&conn, input)
}

/// 删除网课及其时间戳。
#[tauri::command]
pub fn delete_resource(app_handle: AppHandle, id: i64) -> AppResult<()> {
    let conn = database::get_connection(&app_handle)?;
    resources::delete(&conn, id)
}

/// 新建或修改时间戳。
#[tauri::command]
pub fn save_resource_mark(app_handle: AppHandle, input: MarkInput) -> AppResult<Resource> {
    let conn = database::get_connection(&app_handle)?;
    resources::save_mark(&conn, input)
}

/// 删除时间戳，返回更新后的网课。
#[tauri::command]
pub fn delete_resource_mark(app_handle: AppHandle, resource_id: i64, id: i64) -> AppResult<Resource> {
    let conn = database::get_connection(&app_handle)?;
    resources::delete_mark(&conn, id)?;
    resources::get(&conn, resource_id)
}

// --------------------------------------------------------------------------- notes

/// 课程的笔记。
#[tauri::command]
pub fn list_notes(app_handle: AppHandle, course_id: i64) -> AppResult<Vec<Note>> {
    let conn = database::get_connection(&app_handle)?;
    notes::list(&conn, course_id)
}

/// 新建或修改笔记。
#[tauri::command]
pub fn save_note(app_handle: AppHandle, input: NoteInput) -> AppResult<Note> {
    let conn = database::get_connection(&app_handle)?;
    notes::save(&conn, input)
}

/// 删除笔记。
#[tauri::command]
pub fn delete_note(app_handle: AppHandle, id: i64) -> AppResult<()> {
    let conn = database::get_connection(&app_handle)?;
    notes::delete(&conn, id)
}

// --------------------------------------------------------------------------- study state

/// 课程学习状态：当前路径与步骤、上次阅读、上次 AI 学习、进度与近两周活动。
#[tauri::command]
pub fn get_study_state(app_handle: AppHandle, course_id: i64) -> AppResult<StudyState> {
    let conn = database::get_connection(&app_handle)?;
    study::state(&conn, course_id)
}

/// 记录阅读位置（同一资料 10 分钟内只计一次阅读活动）。
#[tauri::command]
pub fn save_reading_position(app_handle: AppHandle, course_id: i64, document_id: i64, page: i64) -> AppResult<()> {
    let conn = database::get_connection(&app_handle)?;
    study::note_reading(&conn, course_id, document_id, page)
}

/// 首页概况：每门课程的进度、今日到期复习、未解决错题和考试倒计时。
#[tauri::command]
pub fn get_dashboard(app_handle: AppHandle) -> AppResult<Vec<CourseDigest>> {
    let conn = database::get_connection(&app_handle)?;
    study::dashboard(&conn, crate::mastery::today())
}
