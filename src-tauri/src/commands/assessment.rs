//! Stage 5 commands: quizzes, grading, the mistake book, the review queue and
//! next-step recommendations.

use crate::ai::AiState;
use crate::assessment::{self, AnswerInput, CreateQuiz, Mistake, Question, Quiz, QuizDetail};
use crate::database::{self, AppPaths};
use crate::error::AppResult;
use crate::mastery::{self, ReviewQueue};
use crate::review::{self, Action, Advice};
use tauri::{AppHandle, State};

/// AI 出题（学习步骤若有未完成的小测则直接返回它）。
#[tauri::command]
pub async fn create_quiz(paths: State<'_, AppPaths>, ai: State<'_, AiState>, input: CreateQuiz) -> AppResult<QuizDetail> {
    assessment::create(paths.inner(), ai.inner(), input).await
}

/// 课程的小测记录。
#[tauri::command]
pub fn list_quizzes(app_handle: AppHandle, course_id: i64) -> AppResult<Vec<Quiz>> {
    let conn = database::get_connection(&app_handle)?;
    assessment::list(&conn, course_id)
}

/// 小测详情；交卷前不包含答案、解析和评分要点。
#[tauri::command]
pub fn get_quiz(app_handle: AppHandle, id: i64) -> AppResult<QuizDetail> {
    let conn = database::get_connection(&app_handle)?;
    assessment::detail(&conn, id)
}

/// 删除小测（已计入的掌握度和复习计划不撤销）。
#[tauri::command]
pub fn delete_quiz(app_handle: AppHandle, id: i64) -> AppResult<()> {
    let conn = database::get_connection(&app_handle)?;
    assessment::delete(&conn, id)
}

/// 保存一道题的答案，或标记跳过。
#[tauri::command]
pub fn save_quiz_answer(app_handle: AppHandle, input: AnswerInput) -> AppResult<Question> {
    let conn = database::get_connection(&app_handle)?;
    assessment::save_answer(&conn, input)
}

/// 记住当前所在的题目。
#[tauri::command]
pub fn set_quiz_position(app_handle: AppHandle, quiz_id: i64, index: i64) -> AppResult<()> {
    let conn = database::get_connection(&app_handle)?;
    assessment::set_position(&conn, quiz_id, index)
}

/// 交卷：客观题判分、主观题 AI 批改；全部有分后更新掌握度、复习计划、错题本和学习步骤。
#[tauri::command]
pub async fn submit_quiz(paths: State<'_, AppPaths>, ai: State<'_, AiState>, id: i64) -> AppResult<QuizDetail> {
    assessment::submit(paths.inner(), ai.inner(), id).await
}

/// 重新让 AI 批改尚未评分的主观题。
#[tauri::command]
pub async fn regrade_quiz(paths: State<'_, AppPaths>, ai: State<'_, AiState>, id: i64) -> AppResult<QuizDetail> {
    assessment::grade_pending(paths.inner(), ai.inner(), id).await
}

/// 对照参考答案自评一道主观题（0–1 分）。
#[tauri::command]
pub fn self_grade_question(app_handle: AppHandle, question_id: i64, score: f64) -> AppResult<QuizDetail> {
    let conn = database::get_connection(&app_handle)?;
    assessment::self_grade(&conn, question_id, score)
}

/// 错题本（可包含已解决的错题）。
#[tauri::command]
pub fn list_mistakes(app_handle: AppHandle, course_id: i64, include_resolved: bool) -> AppResult<Vec<Mistake>> {
    let conn = database::get_connection(&app_handle)?;
    assessment::list_mistakes(&conn, course_id, include_resolved)
}

/// 标记错题已解决，或移回错题本。
#[tauri::command]
pub fn resolve_mistake(app_handle: AppHandle, id: i64, resolved: bool) -> AppResult<()> {
    let conn = database::get_connection(&app_handle)?;
    assessment::resolve_mistake(&conn, id, resolved)
}

/// 复习队列与概况（到期、薄弱、疑难、学过未测、错题）。
#[tauri::command]
pub fn get_review_queue(app_handle: AppHandle, course_id: i64) -> AppResult<ReviewQueue> {
    let conn = database::get_connection(&app_handle)?;
    mastery::queue(&conn, course_id, mastery::today())
}

/// 下一步建议（由学习状态按规则生成，可直接执行）。
#[tauri::command]
pub fn get_recommendations(app_handle: AppHandle, course_id: i64) -> AppResult<Vec<Action>> {
    let conn = database::get_connection(&app_handle)?;
    review::recommendations(&conn, course_id, mastery::today())
}

/// AI 学习建议与优先复习的知识点（只保留本课程存在的知识点）。
#[tauri::command]
pub async fn get_ai_advice(paths: State<'_, AppPaths>, ai: State<'_, AiState>, course_id: i64) -> AppResult<Advice> {
    review::advise(paths.inner(), ai.inner(), course_id).await
}
