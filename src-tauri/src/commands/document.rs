use crate::{
    database::{self, AppPaths},
    documents::{self, Document, DocumentTasks, PageView, Section},
    error::{AppError, AppResult},
    search::{self, SearchResult},
};
use base64::Engine;
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::{ipc::Channel, AppHandle, State};

/// Limits concurrent page-preview renders (each one starts the processor).
pub struct PreviewLimiter(pub tokio::sync::Semaphore);

impl Default for PreviewLimiter {
    fn default() -> Self {
        PreviewLimiter(tokio::sync::Semaphore::new(2))
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportProgress {
    pub document_id: i64,
    pub stage: String,
    pub current: u64,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CourseSection {
    #[serde(flatten)]
    pub section: Section,
    pub filename: String,
    pub file_type: String,
}

fn data_url(path: &Path) -> AppResult<String> {
    let bytes = std::fs::read(path)?;
    let mime = match path.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()).as_deref() {
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        _ => "image/png",
    };
    Ok(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
}

/// 课程的资料列表。
#[tauri::command]
pub fn get_documents(app_handle: AppHandle, course_id: i64) -> AppResult<Vec<Document>> {
    let conn = database::get_connection(&app_handle)?;
    documents::list(&conn, course_id)
}

/// 单份资料。
#[tauri::command]
pub fn get_document(app_handle: AppHandle, id: i64) -> AppResult<Document> {
    let conn = database::get_connection(&app_handle)?;
    documents::get(&conn, id)
}

/// 资料全部页面拼接成的纯文本。
#[tauri::command]
pub fn get_document_content(app_handle: AppHandle, id: i64) -> AppResult<String> {
    let conn = database::get_connection(&app_handle)?;
    documents::body(&conn, id)
}

/// 保存资料并加入持久化后台队列，Channel 通知入队；进度从资料状态查询。
#[tauri::command]
pub async fn import_document(
    app_handle: AppHandle,
    paths: State<'_, AppPaths>,
    tasks: State<'_, DocumentTasks>,
    course_id: i64,
    path: String,
    on_progress: Channel<ImportProgress>,
) -> AppResult<Document> {
    let _registration = tasks.2.lock().await;
    let paths = paths.inner().clone();
    let document = tauri::async_runtime::spawn_blocking(move || {
        let mut conn = paths.connect()?;
        let document = documents::prepare(&mut conn, &paths.documents_dir, course_id, &PathBuf::from(path))?;
        if document.status != "failed" { documents::jobs::enqueue(&mut conn, document.id, false)?; }
        documents::get(&conn, document.id)
    }).await??;
    let _ = on_progress.send(ImportProgress { document_id: document.id, stage: "queued".into(), current: 0, total: 0 });
    documents::jobs::start(app_handle);
    Ok(document)
}

/// Queue a re-parse without replacing the previously usable result.
#[tauri::command]
pub async fn reprocess_document(
    app_handle: AppHandle,
    paths: State<'_, AppPaths>,
    tasks: State<'_, DocumentTasks>,
    id: i64,
    on_progress: Channel<ImportProgress>,
) -> AppResult<Document> {
    let _registration = tasks.2.lock().await;
    let mut conn = paths.connect()?;
    let document = documents::get(&conn,id)?;
    if !documents::locate_copy(&paths.documents_dir,&document).is_file() {
        return Err(AppError::msg("应用内的资料副本已丢失，请删除后重新导入"));
    }
    documents::jobs::enqueue(&mut conn,id,document.indexed_at.is_some())?;
    let queued = documents::get(&conn,id)?;
    let _ = on_progress.send(ImportProgress { document_id: id, stage: "queued".into(), current: 0, total: 0 });
    documents::jobs::start(app_handle);
    Ok(queued)
}

/// 删除资料及其章节、片段和图片（原文件不受影响）。
#[tauri::command]
pub async fn delete_document(paths: State<'_, AppPaths>, tasks: State<'_, DocumentTasks>, id: i64) -> AppResult<()> {
    let _guard = tasks.0.lock().await;
    let _registration = tasks.2.lock().await;
    let paths = paths.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut conn = paths.connect()?;
        documents::delete_document(&mut conn, &paths, id)
    })
    .await?
}

/// 资料的章节树。
#[tauri::command]
pub fn get_document_sections(app_handle: AppHandle, document_id: i64) -> AppResult<Vec<Section>> {
    let conn = database::get_connection(&app_handle)?;
    documents::sections(&conn, document_id)
}

/// 手动调整章节；保留原文及已有引用，和重新解析互斥。
#[tauri::command]
pub async fn edit_document_section(
    paths: State<'_, AppPaths>, tasks: State<'_, DocumentTasks>,
    document_id: i64, section_id: i64, edit: documents::edit::SectionEdit,
) -> AppResult<Vec<Section>> {
    let _guard = tasks.0.lock().await;
    let _registration = tasks.2.lock().await;
    let paths = paths.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        documents::edit::apply(&mut paths.connect()?, document_id, section_id, edit)
    }).await?
}

/// 课程全部资料的章节（用于章节选择）。
#[tauri::command]
pub fn get_course_sections(app_handle: AppHandle, course_id: i64) -> AppResult<Vec<CourseSection>> {
    let conn = database::get_connection(&app_handle)?;
    let mut result = Vec::new();
    for document in documents::list(&conn, course_id)?.into_iter().rev() {
        if document.status != "completed" {
            continue;
        }
        for section in documents::sections(&conn, document.id)? {
            result.push(CourseSection { section, filename: document.filename.clone(), file_type: document.file_type.clone() });
        }
    }
    Ok(result)
}

/// 指定范围的页面 / 单元内容。
#[tauri::command]
pub fn get_document_pages(app_handle: AppHandle, document_id: i64, start: i64, end: i64) -> AppResult<Vec<PageView>> {
    let conn = database::get_connection(&app_handle)?;
    documents::pages(&conn, document_id, start, end)
}

/// PDF 原页面预览图（data URL，按需渲染并缓存）。
#[tauri::command]
pub async fn get_page_preview(
    paths: State<'_, AppPaths>,
    limiter: State<'_, PreviewLimiter>,
    document_id: i64,
    page: i64,
    width: Option<u32>,
) -> AppResult<String> {
    let _permit = limiter.0.acquire().await.map_err(|e| AppError::msg(e.to_string()))?;
    let file = documents::page_preview(paths.inner(), document_id, page, width.unwrap_or(1200)).await?;
    data_url(&file)
}

/// 提取出的图片（data URL）。
#[tauri::command]
pub fn get_document_image(paths: State<'_, AppPaths>, id: i64) -> AppResult<String> {
    let conn = paths.connect()?;
    let path = documents::image_path(&conn, &paths.documents_dir, id)?;
    if !path.exists() {
        return Err(AppError::msg("图片文件已丢失，可重新解析该资料"));
    }
    data_url(&path)
}

/// 课程内关键词检索，返回带高亮的片段和原文位置。
#[tauri::command]
pub fn search_course(
    app_handle: AppHandle,
    course_id: i64,
    query: String,
    document_id: Option<i64>,
    limit: Option<usize>,
) -> AppResult<SearchResult> {
    if query.chars().count() > 200 {
        return Err(AppError::invalid("检索词过长"));
    }
    let conn = database::get_connection(&app_handle)?;
    search::search_course(&conn, course_id, query.trim(), document_id, limit.unwrap_or(50))
}
