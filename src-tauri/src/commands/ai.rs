use crate::ai::{self, client::TestReport, profiles, AiState};
use crate::database::{self, AppPaths};
use crate::error::AppResult;
use crate::settings;
use crate::tutor::{self, chat, Conversation, ConversationDetail, Message};
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

#[tauri::command]
pub async fn send_message(
    paths: State<'_, AppPaths>,
    ai: State<'_, AiState>,
    input: chat::SendInput,
    on_event: Channel<chat::ChatEvent>,
) -> AppResult<Message> {
    chat::run_turn(paths.inner(), ai.inner(), input.conversation_id, chat::qa_plan, &input.content, input.regenerate, move |event| {
        let _ = on_event.send(event);
    })
    .await
}

#[tauri::command]
pub fn cancel_generation(ai: State<'_, AiState>, request_id: String) -> bool {
    ai.cancel(&request_id)
}
