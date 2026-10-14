use crate::ai::{self, client::TestReport, profiles, AiState};
use crate::database::{self, AppPaths};
use crate::error::AppResult;
use crate::settings;
use crate::tutor::{self, chat, teach, Conversation, ConversationDetail, Message};
use tauri::{ipc::Channel, AppHandle, State};

/// 所有模型配置（不含 API Key，只有首尾几位提示）。
#[tauri::command]
pub fn list_model_profiles(app_handle: AppHandle) -> AppResult<Vec<profiles::ModelProfile>> {
    let conn = database::get_connection(&app_handle)?;
    profiles::list(&conn)
}

/// 新建或修改模型配置；API Key 存入系统凭据管理器（不可用时回退本地并返回提示）。
#[tauri::command]
pub fn save_model_profile(app_handle: AppHandle, ai: State<'_, AiState>, input: profiles::ProfileInput) -> AppResult<profiles::SaveResult> {
    let mut conn = database::get_connection(&app_handle)?;
    profiles::save(&mut conn, ai.secrets.as_ref(), input)
}

/// 删除模型配置及其保存的 API Key。
#[tauri::command]
pub fn delete_model_profile(app_handle: AppHandle, ai: State<'_, AiState>, id: i64) -> AppResult<()> {
    let mut conn = database::get_connection(&app_handle)?;
    profiles::delete(&mut conn, ai.secrets.as_ref(), id)
}

/// 设为默认模型。
#[tauri::command]
pub fn set_default_model_profile(app_handle: AppHandle, id: i64) -> AppResult<()> {
    let conn = database::get_connection(&app_handle)?;
    profiles::set_default(&conn, id)
}

/// 测试连接（使用表单中尚未保存的值；未填写 API Key 时使用已保存的）。
#[tauri::command]
pub async fn test_model_profile(paths: State<'_, AppPaths>, ai: State<'_, AiState>, input: profiles::ProfileInput) -> AppResult<TestReport> {
    let key = match (input.api_key.clone(), input.id) {
        (Some(key), _) if !key.trim().is_empty() => Some(key.trim().to_string()),
        (None, Some(id)) => {
            let conn = paths.connect()?;
            profiles::resolve(&conn, ai.secrets.as_ref(), Some(id))?.api_key
        }
        _ => None,
    };
    let profile = profiles::from_input(&input, key)?;
    let report = ai.gateway.test(&profile).await?;
    let conn = paths.connect()?;
    ai::record_usage(&conn, input.id, "test", 12, 4, true)?;
    Ok(report)
}

/// Token 用量统计（今天 / 累计）。
#[tauri::command]
pub fn get_ai_usage(app_handle: AppHandle) -> AppResult<ai::UsageSummary> {
    let conn = database::get_connection(&app_handle)?;
    ai::usage_summary(&conn)
}

/// 课程的全部对话（AI 问答与 AI 学习）。
#[tauri::command]
pub fn list_conversations(app_handle: AppHandle, course_id: i64) -> AppResult<Vec<Conversation>> {
    let conn = database::get_connection(&app_handle)?;
    tutor::list(&conn, course_id)
}

/// 新建问答对话。
#[tauri::command]
pub fn create_conversation(app_handle: AppHandle, input: tutor::CreateConversation) -> AppResult<Conversation> {
    let conn = database::get_connection(&app_handle)?;
    let default_mode = settings::load(&conn)?.context.mode;
    tutor::create(&conn, input, default_mode)
}

/// 对话及其全部消息。
#[tauri::command]
pub fn get_conversation(app_handle: AppHandle, id: i64) -> AppResult<ConversationDetail> {
    let conn = database::get_connection(&app_handle)?;
    tutor::detail(&conn, id)
}

/// 修改标题、上下文模式、资料范围或使用的模型。
#[tauri::command]
pub fn update_conversation(app_handle: AppHandle, input: tutor::UpdateConversation) -> AppResult<Conversation> {
    let conn = database::get_connection(&app_handle)?;
    tutor::update(&conn, input)
}

/// 删除对话及其消息。
#[tauri::command]
pub fn delete_conversation(app_handle: AppHandle, id: i64) -> AppResult<()> {
    let conn = database::get_connection(&app_handle)?;
    tutor::delete(&conn, id)
}

/// 发送消息并流式返回回答：问答对话直接回答；学习对话（从零学习 / 正常复习 / 考前冲刺）按动作（继续、举例、跳过等）组织讲解。
#[tauri::command]
pub async fn send_message(
    paths: State<'_, AppPaths>,
    ai: State<'_, AiState>,
    input: chat::SendInput,
    on_event: Channel<chat::ChatEvent>,
) -> AppResult<Message> {
    let emit = move |event| {
        let _ = on_event.send(event);
    };
    let teaching = {
        let conn = paths.connect()?;
        let conversation = tutor::get(&conn, input.conversation_id)?;
        if conversation.mode == "qa" {
            None
        } else if input.regenerate {
            Some(teach::prepare_regenerate(&conn, input.conversation_id)?)
        } else {
            Some((input.action.clone().unwrap_or_else(|| "ask".into()), input.target))
        }
    };
    match teaching {
        None => chat::run_turn(paths.inner(), ai.inner(), input.conversation_id, chat::qa_plan, &input.content, input.regenerate, emit).await,
        Some((action, target)) => {
            let plan = move |conn: &rusqlite::Connection, conversation: &Conversation, text: &str| teach::plan(conn, conversation, &action, target, text);
            chat::run_turn(paths.inner(), ai.inner(), input.conversation_id, plan, &input.content, input.regenerate, emit).await
        }
    }
}

/// 打开（或新建）某一章节 / 网课在指定学习模式下的学习对话。
#[tauri::command]
pub fn start_teaching(app_handle: AppHandle, input: teach::StartTeaching) -> AppResult<Conversation> {
    let conn = database::get_connection(&app_handle)?;
    let default_mode = settings::load(&conn)?.context.mode;
    teach::start(&conn, input, default_mode)
}

/// “从零学习”的分段及每段状态（已讲 / 跳过 / 下一段 / 未讲）。
#[tauri::command]
pub fn get_lesson_outline(app_handle: AppHandle, conversation_id: i64) -> AppResult<Vec<teach::LessonPart>> {
    let conn = database::get_connection(&app_handle)?;
    teach::outline(&conn, conversation_id)
}

/// 标记学完：相关知识点记为“学过”，关联的学习步骤完成。
#[tauri::command]
pub fn mark_lesson_finished(app_handle: AppHandle, conversation_id: i64) -> AppResult<Conversation> {
    let conn = database::get_connection(&app_handle)?;
    teach::mark_finished(&conn, conversation_id)
}

/// 停止正在生成的回答（保留已生成的内容）。
#[tauri::command]
pub fn cancel_generation(ai: State<'_, AiState>, request_id: String) -> bool {
    ai.cancel(&request_id)
}
