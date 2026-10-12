// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod database;
mod models;
mod documents;

fn main() {
    tauri::Builder::default()
        .manage(documents::DocumentTasks::default())
        .setup(|app| {
            // Initialize database
            let app_handle = app.handle();
            database::init_database(&app_handle)?;

            Ok(())
        })
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::course::create_course,
            commands::course::get_courses,
            commands::course::update_course,
            commands::course::delete_course,
            commands::document::get_documents,
            commands::document::get_document_content,
            commands::document::import_document,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
