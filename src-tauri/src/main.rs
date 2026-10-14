// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod database;
mod documents;
mod error;
mod models;
mod search;
mod settings;
mod sidecar;
#[cfg(test)]
mod test_support;

use tauri::Manager;
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(documents::DocumentTasks::default())
        .manage(commands::document::PreviewLimiter::default())
        .setup(|app| {
            let paths = database::AppPaths::resolve(app.handle())?;
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
            }
            app.manage(paths);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::get_app_settings,
            commands::app::update_app_settings,
            commands::app::get_app_info,
            commands::app::open_data_dir,
            commands::app::save_last_location,
            commands::app::get_last_location,
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
            commands::document::get_course_sections,
            commands::document::get_document_pages,
            commands::document::get_page_preview,
            commands::document::get_document_image,
            commands::document::search_course,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
