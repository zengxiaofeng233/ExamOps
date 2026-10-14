//! Stage 4 commands: knowledge points, learning paths, course videos, notes
//! and the study state.

use crate::ai::AiState;
use crate::database::{self, AppPaths};
use crate::error::{AppError, AppResult};
use crate::knowledge::{self, ExtractResult, KnowledgeInput, KnowledgePoint};
use crate::notes::{self, Note, NoteInput};
use crate::paths::{self, LearningPath, PathDetail, PathInput, PathStep, RecommendInput, StepInput, TemplateInput};
use crate::resources::{self, MarkInput, Resource, ResourceInput};
use crate::study::{self, StudyState};
use tauri::{AppHandle, State};

// --------------------------------------------------------------------------- knowledge points

#[tauri::command]
pub fn list_knowledge_points(app_handle: AppHandle, course_id: i64) -> AppResult<Vec<KnowledgePoint>> {
    let conn = database::get_connection(&app_handle)?;
    knowledge::list(&conn, course_id)
}

#[tauri::command]
pub fn save_knowledge_point(app_handle: AppHandle, input: KnowledgeInput) -> AppResult<KnowledgePoint> {
    let conn = database::get_connection(&app_handle)?;
    knowledge::save(&conn, input)
}

#[tauri::command]
pub fn delete_knowledge_point(app_handle: AppHandle, id: i64) -> AppResult<()> {
    let conn = database::get_connection(&app_handle)?;
    knowledge::delete(&conn, id)
}

#[tauri::command]
pub fn set_knowledge_point_difficult(app_handle: AppHandle, id: i64, difficult: bool) -> AppResult<KnowledgePoint> {
    let conn = database::get_connection(&app_handle)?;
    knowledge::set_difficult(&conn, id, difficult)
}

/// "学过" can be set or reset by the student; "已掌握" is only granted by quiz results.
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

#[tauri::command]
pub fn generate_knowledge_points(app_handle: AppHandle, course_id: i64, document_id: Option<i64>) -> AppResult<usize> {
    let conn = database::get_connection(&app_handle)?;
    knowledge::from_headings(&conn, course_id, document_id)
}

#[tauri::command]
pub async fn extract_knowledge_points(paths: State<'_, AppPaths>, ai: State<'_, AiState>, course_id: i64, section_id: i64) -> AppResult<ExtractResult> {
    knowledge::extract_with_ai(paths.inner(), ai.inner(), course_id, section_id).await
}

// --------------------------------------------------------------------------- learning paths

#[tauri::command]
pub fn list_learning_paths(app_handle: AppHandle, course_id: i64) -> AppResult<Vec<LearningPath>> {
    let conn = database::get_connection(&app_handle)?;
    paths::list(&conn, course_id)
}

#[tauri::command]
pub fn get_learning_path(app_handle: AppHandle, id: i64) -> AppResult<PathDetail> {
    let conn = database::get_connection(&app_handle)?;
    paths::detail(&conn, id)
}

#[tauri::command]
pub fn save_learning_path(app_handle: AppHandle, input: PathInput) -> AppResult<LearningPath> {
    let conn = database::get_connection(&app_handle)?;
    paths::save_path(&conn, input)
}

#[tauri::command]
pub fn delete_learning_path(app_handle: AppHandle, id: i64) -> AppResult<()> {
    let conn = database::get_connection(&app_handle)?;
    paths::delete_path(&conn, id)
}

#[tauri::command]
pub fn activate_learning_path(app_handle: AppHandle, course_id: i64, id: i64) -> AppResult<StudyState> {
    let conn = database::get_connection(&app_handle)?;
    if paths::get_path(&conn, id)?.course_id != course_id {
        return Err(AppError::invalid("学习路径不属于该课程"));
    }
    study::set_active_path(&conn, course_id, Some(id))?;
    study::state(&conn, course_id)
}

#[tauri::command]
pub fn create_path_from_template(app_handle: AppHandle, input: TemplateInput) -> AppResult<PathDetail> {
    let mut conn = database::get_connection(&app_handle)?;
    paths::from_template(&mut conn, input)
}

#[tauri::command]
pub async fn recommend_learning_path(paths: State<'_, AppPaths>, ai: State<'_, AiState>, input: RecommendInput) -> AppResult<PathDetail> {
    paths::recommend_with_ai(paths.inner(), ai.inner(), input).await
}

#[tauri::command]
pub fn save_path_step(app_handle: AppHandle, input: StepInput) -> AppResult<PathStep> {
    let mut conn = database::get_connection(&app_handle)?;
    paths::save_step(&mut conn, input)
}

#[tauri::command]
pub fn delete_path_step(app_handle: AppHandle, id: i64) -> AppResult<()> {
    let mut conn = database::get_connection(&app_handle)?;
    paths::delete_step(&mut conn, id)
}

#[tauri::command]
pub fn reorder_path_steps(app_handle: AppHandle, path_id: i64, step_ids: Vec<i64>) -> AppResult<Vec<PathStep>> {
    let mut conn = database::get_connection(&app_handle)?;
    paths::reorder(&mut conn, path_id, &step_ids)
}

#[tauri::command]
pub fn set_path_step_status(app_handle: AppHandle, id: i64, status: String, note: Option<String>) -> AppResult<PathStep> {
    let conn = database::get_connection(&app_handle)?;
    paths::set_status(&conn, id, &status, note.as_deref())
}

// --------------------------------------------------------------------------- course videos

#[tauri::command]
pub fn list_resources(app_handle: AppHandle, course_id: i64) -> AppResult<Vec<Resource>> {
    let conn = database::get_connection(&app_handle)?;
    resources::list(&conn, course_id)
}

#[tauri::command]
pub fn save_resource(app_handle: AppHandle, input: ResourceInput) -> AppResult<Resource> {
    let conn = database::get_connection(&app_handle)?;
    resources::save(&conn, input)
}

#[tauri::command]
pub fn delete_resource(app_handle: AppHandle, id: i64) -> AppResult<()> {
    let conn = database::get_connection(&app_handle)?;
    resources::delete(&conn, id)
}

#[tauri::command]
pub fn save_resource_mark(app_handle: AppHandle, input: MarkInput) -> AppResult<Resource> {
    let conn = database::get_connection(&app_handle)?;
    resources::save_mark(&conn, input)
}

#[tauri::command]
pub fn delete_resource_mark(app_handle: AppHandle, resource_id: i64, id: i64) -> AppResult<Resource> {
    let conn = database::get_connection(&app_handle)?;
    resources::delete_mark(&conn, id)?;
    resources::get(&conn, resource_id)
}

// --------------------------------------------------------------------------- notes

#[tauri::command]
pub fn list_notes(app_handle: AppHandle, course_id: i64) -> AppResult<Vec<Note>> {
    let conn = database::get_connection(&app_handle)?;
    notes::list(&conn, course_id)
}

#[tauri::command]
pub fn save_note(app_handle: AppHandle, input: NoteInput) -> AppResult<Note> {
    let conn = database::get_connection(&app_handle)?;
    notes::save(&conn, input)
}

#[tauri::command]
pub fn delete_note(app_handle: AppHandle, id: i64) -> AppResult<()> {
    let conn = database::get_connection(&app_handle)?;
    notes::delete(&conn, id)
}

// --------------------------------------------------------------------------- study state

#[tauri::command]
pub fn get_study_state(app_handle: AppHandle, course_id: i64) -> AppResult<StudyState> {
    let conn = database::get_connection(&app_handle)?;
    study::state(&conn, course_id)
}

#[tauri::command]
pub fn save_reading_position(app_handle: AppHandle, course_id: i64, document_id: i64, page: i64) -> AppResult<()> {
    let conn = database::get_connection(&app_handle)?;
    study::note_reading(&conn, course_id, document_id, page)
}
