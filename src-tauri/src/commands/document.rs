use crate::{
    database,
    documents::{self, Document, DocumentTasks},
};
use std::path::PathBuf;
use tauri::{AppHandle, Manager, State};

pub fn storage_root(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("documents"))
}

#[tauri::command]
pub fn get_documents(app_handle: AppHandle, course_id: i64) -> Result<Vec<Document>, String> {
    let conn = database::get_connection(&app_handle).map_err(|e| e.to_string())?;
    documents::list(&conn, course_id)
}

#[tauri::command]
pub fn get_document_content(app_handle: AppHandle, id: i64) -> Result<String, String> {
    let conn = database::get_connection(&app_handle).map_err(|e| e.to_string())?;
    documents::body(&conn, id)
}

#[tauri::command]
pub async fn import_document(
    app_handle: AppHandle,
    tasks: State<'_, DocumentTasks>,
    course_id: i64,
    path: String,
) -> Result<Document, String> {
    let _guard = tasks.0.lock().await;
    let app = app_handle.clone();
    let document = tauri::async_runtime::spawn_blocking(move || {
        let mut conn = database::get_connection(&app).map_err(|e| e.to_string())?;
        documents::prepare(
            &mut conn,
            &storage_root(&app)?,
            course_id,
            &PathBuf::from(path),
        )
    })
    .await
    .map_err(|e| e.to_string())??;
    if document.status == "failed" {
        return Ok(document);
    }
    let result = documents::extract(&PathBuf::from(&document.file_path)).await;
    let mut conn = database::get_connection(&app_handle).map_err(|e| e.to_string())?;
    documents::finish(&mut conn, document.id, result)
}
