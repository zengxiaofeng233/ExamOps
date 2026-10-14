//! Stage 5 commands: quizzes, grading, the mistake book, the review queue and
//! next-step recommendations.

use crate::ai::AiState;
use crate::assessment::{self, AnswerInput, CreateQuiz, Mistake, Question, Quiz, QuizDetail};
use crate::database::{self, AppPaths};
use crate::error::AppResult;
use crate::mastery::{self, ReviewQueue};
use crate::review::{self, Action, Advice};
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn create_quiz(paths: State<'_, AppPaths>, ai: State<'_, AiState>, input: CreateQuiz) -> AppResult<QuizDetail> {
    assessment::create(paths.inner(), ai.inner(), input).await
}

#[tauri::command]
pub fn list_quizzes(app_handle: AppHandle, course_id: i64) -> AppResult<Vec<Quiz>> {
    let conn = database::get_connection(&app_handle)?;
    assessment::list(&conn, course_id)
}

#[tauri::command]
pub fn get_quiz(app_handle: AppHandle, id: i64) -> AppResult<QuizDetail> {
    let conn = database::get_connection(&app_handle)?;
    assessment::detail(&conn, id)
}

#[tauri::command]
pub fn delete_quiz(app_handle: AppHandle, id: i64) -> AppResult<()> {
    let conn = database::get_connection(&app_handle)?;
    assessment::delete(&conn, id)
}

#[tauri::command]
pub fn save_quiz_answer(app_handle: AppHandle, input: AnswerInput) -> AppResult<Question> {
    let conn = database::get_connection(&app_handle)?;
    assessment::save_answer(&conn, input)
}

#[tauri::command]
pub fn set_quiz_position(app_handle: AppHandle, quiz_id: i64, index: i64) -> AppResult<()> {
    let conn = database::get_connection(&app_handle)?;
    assessment::set_position(&conn, quiz_id, index)
}

#[tauri::command]
pub async fn submit_quiz(paths: State<'_, AppPaths>, ai: State<'_, AiState>, id: i64) -> AppResult<QuizDetail> {
    assessment::submit(paths.inner(), ai.inner(), id).await
}

#[tauri::command]
pub async fn regrade_quiz(paths: State<'_, AppPaths>, ai: State<'_, AiState>, id: i64) -> AppResult<QuizDetail> {
    assessment::grade_pending(paths.inner(), ai.inner(), id).await
}

#[tauri::command]
pub fn self_grade_question(app_handle: AppHandle, question_id: i64, score: f64) -> AppResult<QuizDetail> {
    let conn = database::get_connection(&app_handle)?;
    assessment::self_grade(&conn, question_id, score)
}

#[tauri::command]
pub fn list_mistakes(app_handle: AppHandle, course_id: i64, include_resolved: bool) -> AppResult<Vec<Mistake>> {
    let conn = database::get_connection(&app_handle)?;
    assessment::list_mistakes(&conn, course_id, include_resolved)
}

#[tauri::command]
pub fn resolve_mistake(app_handle: AppHandle, id: i64, resolved: bool) -> AppResult<()> {
    let conn = database::get_connection(&app_handle)?;
    assessment::resolve_mistake(&conn, id, resolved)
}

#[tauri::command]
pub fn get_review_queue(app_handle: AppHandle, course_id: i64) -> AppResult<ReviewQueue> {
    let conn = database::get_connection(&app_handle)?;
    mastery::queue(&conn, course_id, mastery::today())
}

#[tauri::command]
pub fn get_recommendations(app_handle: AppHandle, course_id: i64) -> AppResult<Vec<Action>> {
    let conn = database::get_connection(&app_handle)?;
    review::recommendations(&conn, course_id, mastery::today())
}

#[tauri::command]
pub async fn get_ai_advice(paths: State<'_, AppPaths>, ai: State<'_, AiState>, course_id: i64) -> AppResult<Advice> {
    review::advise(paths.inner(), ai.inner(), course_id).await
}
