use crate::ai::{self, client::TestReport, profiles, AiState};
use crate::database::{self, AppPaths};
use crate::error::AppResult;
use crate::settings;
use crate::tutor::{self, chat, teach, Conversation, ConversationDetail, Message};
use tauri::{ipc::Channel, AppHandle, State};

#[tauri::command]
pub fn list_model_profiles(app_handle: AppHandle) -> AppResult<Vec<profiles::ModelProfile>> {
    let conn = database::get_connection(&app_handle)?;
    profiles::list(&conn)
}

#[tauri::command]
pub fn save_model_profile(app_handle: AppHandle, ai: State<'_, AiState>, input: profiles::ProfileInput) -> AppResult<profiles::SaveResult> {
    let mut conn = database::get_connection(&app_handle)?;
    profiles::save(&mut conn, ai.secrets.as_ref(), input)
}

#[tauri::command]
pub fn delete_model_profile(app_handle: AppHandle, ai: State<'_, AiState>, id: i64) -> AppResult<()> {
    let mut conn = database::get_connection(&app_handle)?;
    profiles::delete(&mut conn, ai.secrets.as_ref(), id)
}

#[tauri::command]
pub fn set_default_model_profile(app_handle: AppHandle, id: i64) -> AppResult<()> {
    let conn = database::get_connection(&app_handle)?;
    profiles::set_default(&conn, id)
}

/// Tests unsaved form values; a missing key falls back to the stored one.
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

#[tauri::command]
pub fn get_ai_usage(app_handle: AppHandle) -> AppResult<ai::UsageSummary> {
    let conn = database::get_connection(&app_handle)?;
    ai::usage_summary(&conn)
}

#[tauri::command]
pub fn list_conversations(app_handle: AppHandle, course_id: i64) -> AppResult<Vec<Conversation>> {
    let conn = database::get_connection(&app_handle)?;
    tutor::list(&conn, course_id)
}

#[tauri::command]
pub fn create_conversation(app_handle: AppHandle, input: tutor::CreateConversation) -> AppResult<Conversation> {
    let conn = database::get_connection(&app_handle)?;
    let default_mode = settings::load(&conn)?.context.mode;
    tutor::create(&conn, input, default_mode)
}

#[tauri::command]
pub fn get_conversation(app_handle: AppHandle, id: i64) -> AppResult<ConversationDetail> {
    let conn = database::get_connection(&app_handle)?;
    tutor::detail(&conn, id)
}

#[tauri::command]
pub fn update_conversation(app_handle: AppHandle, input: tutor::UpdateConversation) -> AppResult<Conversation> {
    let conn = database::get_connection(&app_handle)?;
    tutor::update(&conn, input)
}

#[tauri::command]
pub fn delete_conversation(app_handle: AppHandle, id: i64) -> AppResult<()> {
    let conn = database::get_connection(&app_handle)?;
    tutor::delete(&conn, id)
}

/// Q&A conversations answer questions; teaching conversations (learn / review /
/// sprint) build a teaching plan for the requested action.
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

/// Opens (or creates) the teaching conversation for a chapter / video and mode.
#[tauri::command]
pub fn start_teaching(app_handle: AppHandle, input: teach::StartTeaching) -> AppResult<Conversation> {
    let conn = database::get_connection(&app_handle)?;
    let default_mode = settings::load(&conn)?.context.mode;
    teach::start(&conn, input, default_mode)
}

#[tauri::command]
pub fn get_lesson_outline(app_handle: AppHandle, conversation_id: i64) -> AppResult<Vec<teach::LessonPart>> {
    let conn = database::get_connection(&app_handle)?;
    teach::outline(&conn, conversation_id)
}

#[tauri::command]
pub fn mark_lesson_finished(app_handle: AppHandle, conversation_id: i64) -> AppResult<Conversation> {
    let conn = database::get_connection(&app_handle)?;
    teach::mark_finished(&conn, conversation_id)
}

#[tauri::command]
pub fn cancel_generation(ai: State<'_, AiState>, request_id: String) -> bool {
    ai.cancel(&request_id)
}
