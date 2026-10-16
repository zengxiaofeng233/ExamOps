// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ai;
mod assessment;
mod bilibili;
mod commands;
mod context;
mod database;
mod documents;
mod error;
mod knowledge;
mod mastery;
mod models;
mod notes;
mod paths;
mod resources;
mod review;
mod search;
mod security;
mod settings;
mod sidecar;
mod study;
mod storage;
mod tutor;
mod updates;
#[cfg(test)]
mod test_support;

use std::sync::Arc;
use tauri::Manager;
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(documents::DocumentTasks::default())
        .manage(bilibili::BilibiliState::default())
        .manage(commands::document::PreviewLimiter::default())
        .manage(ai::AiState::new(Arc::new(security::KeyringStore)))
        .setup(|app| {
            let paths = match database::AppPaths::resolve(app.handle()) {
                Ok(paths) => paths,
                Err(error) => {
                    app.dialog().message(&error).title("ExamOps 无法启动")
                        .kind(MessageDialogKind::Error).blocking_show();
                    return Err(error.into());
                }
            };
            if let Err(error) = database::init_database(&paths) {
                // Without a working database nothing else can run; tell the user why.
                app.dialog()
                    .message(format!(
                        "{error}\n\n数据目录：{}",
                        paths.data_dir.to_string_lossy()
                    ))
                    .title("ExamOps 无法启动")
                    .kind(MessageDialogKind::Error)
                    .blocking_show();
                return Err(error.into());
            }
            // Phase-one imports only stored plain text: index them for search right away.
            if let Ok(mut conn) = paths.connect() {
                let _ = documents::backfill_legacy(&mut conn);
                let _ = tutor::recover_interrupted(&conn);
            }
            app.manage(paths);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::bilibili::get_bilibili_session,
            commands::bilibili::start_bilibili_login,
            commands::bilibili::poll_bilibili_login,
            commands::bilibili::cancel_bilibili_login,
            commands::bilibili::forget_bilibili_login,
            commands::bilibili::fetch_bilibili_text,
            commands::app::get_app_settings,
            commands::app::update_app_settings,
            commands::app::get_app_info,
            commands::app::check_for_updates,
            commands::app::open_data_dir,
            commands::app::get_storage_info,
            commands::app::schedule_data_migration,
            commands::app::cleanup_previous_data,
            commands::app::open_external_url,
            commands::app::save_last_location,
            commands::app::get_last_location,
            commands::app::get_third_party_notices,
            commands::course::create_course,
            commands::course::get_courses,
            commands::course::get_course,
            commands::course::update_course,
            commands::course::delete_course,
            commands::document::get_documents,
            commands::document::get_document,
            commands::document::get_document_content,
            commands::document::import_document,
            commands::document::reprocess_document,
            commands::document::delete_document,
            commands::document::get_document_sections,
            commands::document::edit_document_section,
            commands::document::get_course_sections,
            commands::document::get_document_pages,
            commands::document::get_page_preview,
            commands::document::get_document_image,
            commands::document::search_course,
            commands::ai::list_model_profiles,
            commands::ai::save_model_profile,
            commands::ai::delete_model_profile,
            commands::ai::set_default_model_profile,
            commands::ai::test_model_profile,
            commands::ai::fetch_available_models,
            commands::ai::get_ai_usage,
            commands::ai::list_conversations,
            commands::ai::create_conversation,
            commands::ai::get_conversation,
            commands::ai::update_conversation,
            commands::ai::delete_conversation,
            commands::ai::send_message,
            commands::ai::cancel_generation,
            commands::ai::start_teaching,
            commands::ai::get_lesson_outline,
            commands::ai::mark_lesson_finished,
            commands::study::list_knowledge_points,
            commands::study::save_knowledge_point,
            commands::study::delete_knowledge_point,
            commands::study::set_knowledge_point_difficult,
            commands::study::set_knowledge_point_status,
            commands::study::generate_knowledge_points,
            commands::study::extract_knowledge_points,
            commands::study::list_learning_paths,
            commands::study::get_learning_path,
            commands::study::save_learning_path,
            commands::study::delete_learning_path,
            commands::study::activate_learning_path,
            commands::study::create_path_from_template,
            commands::study::recommend_learning_path,
            commands::study::save_path_step,
            commands::study::delete_path_step,
            commands::study::reorder_path_steps,
            commands::study::set_path_step_status,
            commands::study::list_resources,
            commands::study::save_resource,
            commands::study::delete_resource,
            commands::study::save_resource_mark,
            commands::study::delete_resource_mark,
            commands::study::list_notes,
            commands::study::save_note,
            commands::study::delete_note,
            commands::study::get_study_state,
            commands::study::save_reading_position,
            commands::study::get_dashboard,
            commands::assessment::create_quiz,
            commands::assessment::list_quizzes,
            commands::assessment::get_quiz,
            commands::assessment::delete_quiz,
            commands::assessment::save_quiz_answer,
            commands::assessment::set_quiz_position,
            commands::assessment::submit_quiz,
            commands::assessment::regrade_quiz,
            commands::assessment::self_grade_question,
            commands::assessment::list_mistakes,
            commands::assessment::resolve_mistake,
            commands::assessment::get_review_queue,
            commands::assessment::get_recommendations,
            commands::assessment::get_ai_advice,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
