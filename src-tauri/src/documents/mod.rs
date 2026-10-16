//! Document Engine: import, structured parsing, storage, reading and deletion.

pub mod index;
pub mod edit;
pub mod parse;

use crate::error::{AppError, AppResult, OptionalRow};
use parse::{Extraction, ParseProgress, ParsedBlock};
use rusqlite::{params, Connection};
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Serializes imports, re-parsing and deletions (one heavy job at a time).
#[derive(Default)]
pub struct DocumentTasks(pub tokio::sync::Mutex<()>);

pub const SUPPORTED: [&str; 6] = ["pdf", "docx", "pptx", "txt", "md", "markdown"];

#[derive(Debug, Clone, Serialize)]
pub struct Document {
    pub id: i64,
    pub course_id: i64,
    pub filename: String,
    pub file_type: String,
    pub file_path: String,
    pub file_size: i64,
    pub status: String,
    pub created_at: String,
    pub error: Option<String>,
    pub page_count: i64,
    pub loc_kind: String,
    pub char_count: i64,
    pub structure_source: String,
    pub warnings: Vec<String>,
    pub indexed_at: Option<String>,
    pub section_count: i64,
    pub image_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Section {
    pub id: i64,
    pub document_id: i64,
    pub parent_id: Option<i64>,
    pub ordinal: i64,
    pub level: i64,
    pub title: String,
    pub page_start: i64,
    pub page_end: i64,
    pub char_count: i64,
    pub loc_label: String,
    pub block_start: i64,
    pub block_end: i64,
    pub source: String,
    pub user_edited: bool,
    pub locked: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImageInfo {
    pub id: i64,
    pub page_number: i64,
    pub ordinal: i64,
    pub width: i64,
    pub height: i64,
    pub ocr_text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PageView {
    pub page_number: i64,
    pub label: String,
    pub loc_start: i64,
    pub loc_end: i64,
    pub blocks: Vec<ParsedBlock>,
    pub has_ocr: bool,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub images: Vec<ImageInfo>,
}

const SELECT: &str = "SELECT d.id, d.course_id, d.filename, d.file_type, d.file_path,
    d.file_size, d.status, d.created_at, c.error, d.page_count, d.loc_kind, d.char_count,
    d.structure_source, d.warnings, d.indexed_at,
    (SELECT COUNT(*) FROM document_sections s WHERE s.document_id = d.id),
    (SELECT COUNT(*) FROM document_images i WHERE i.document_id = d.id)
    FROM documents d LEFT JOIN document_contents c ON c.document_id = d.id";

fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Document> {
    let warnings: String = row.get(13)?;
    Ok(Document {
        id: row.get(0)?,
        course_id: row.get(1)?,
        filename: row.get(2)?,
        file_type: row.get(3)?,
        file_path: row.get(4)?,
        file_size: row.get(5)?,
        status: row.get(6)?,
        created_at: row.get(7)?,
        error: row.get(8)?,
        page_count: row.get(9)?,
        loc_kind: row.get(10)?,
        char_count: row.get(11)?,
        structure_source: row.get(12)?,
        warnings: serde_json::from_str(&warnings).unwrap_or_default(),
        indexed_at: row.get(14)?,
        section_count: row.get(15)?,
        image_count: row.get(16)?,
    })
}

pub fn list(conn: &Connection, course_id: i64) -> AppResult<Vec<Document>> {
    let mut stmt = conn.prepare(&format!("{SELECT} WHERE d.course_id = ?1 ORDER BY d.id DESC"))?;
    let rows = stmt.query_map([course_id], from_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn get(conn: &Connection, id: i64) -> AppResult<Document> {
    conn.query_row(&format!("{SELECT} WHERE d.id = ?1"), [id], from_row)
        .or_not_found("资料")
}

/// Plain extracted text (phase-one reader).
pub fn body(conn: &Connection, id: i64) -> AppResult<String> {
    conn.query_row("SELECT content FROM document_contents WHERE document_id = ?1", [id], |r| r.get(0))
        .or_not_found("资料正文")
}

/// Directory holding a document's extracted figures.
pub fn figures_dir(root: &Path, course_id: i64, document_id: i64) -> PathBuf {
    root.join(course_id.to_string()).join(format!("{document_id}_figures"))
}

/// Where the application copy of a document is created: named after
/// application-owned ids, never after the user's file name.
fn copy_path(root: &Path, course_id: i64, document_id: i64, file_type: &str) -> PathBuf {
    root.join(course_id.to_string()).join(format!("{document_id}.{file_type}"))
}

/// The application copy of a document. The database stores absolute paths, so
/// when the data folder has been restored on another computer or under another
/// Windows user the stored path no longer exists; the copy is then found again
/// under the current data directory.
pub fn locate_copy(root: &Path, document: &Document) -> PathBuf {
    let stored = PathBuf::from(&document.file_path);
    if !stored.is_file() {
        let derived = copy_path(root, document.course_id, document.id, &document.file_type);
        if derived.is_file() {
            return derived;
        }
    }
    stored
}

pub fn stem(filename: &str) -> String {
    Path::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(filename)
        .to_string()
}

/// Copies the user's file into application storage and registers it as `processing`.
pub fn prepare(conn: &mut Connection, root: &Path, course_id: i64, source: &Path) -> AppResult<Document> {
    let file_type = source
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    if !SUPPORTED.contains(&file_type.as_str()) {
        return Err(AppError::invalid("支持 PDF、DOCX、PPTX、TXT 和 Markdown 文件"));
    }
    let metadata = source.metadata().map_err(|e| AppError::invalid(format!("无法读取文件：{e}")))?;
    if !metadata.is_file() {
        return Err(AppError::invalid("请选择文件"));
    }
    if metadata.len() == 0 {
        return Err(AppError::invalid("文件为空"));
    }
    let filename = source
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| AppError::invalid("文件名无效"))?;
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO documents (course_id, filename, file_type, file_path, file_size, status, loc_kind)
        VALUES (?1, ?2, ?3, '', ?4, 'processing', ?5)",
        params![
            course_id,
            filename,
            file_type,
            metadata.len() as i64,
            match file_type.as_str() {
                "pdf" => "page",
                "pptx" => "slide",
                "docx" => "paragraph",
                _ => "line",
            }
        ],
    )?;
    let id = tx.last_insert_rowid();
    let target = copy_path(root, course_id, id, &file_type);
    tx.execute(
        "UPDATE documents SET file_path = ?1 WHERE id = ?2",
        params![target.to_string_lossy(), id],
    )?;
    tx.execute("INSERT INTO document_contents (document_id) VALUES (?1)", [id])?;
    tx.commit()?;
    let copied = (|| {
        fs::create_dir_all(target.parent().expect("target has a parent"))?;
        fs::copy(source, &target)?;
        Ok::<(), std::io::Error>(())
    })();
    if let Err(error) = copied {
        mark_failed(conn, id, &format!("复制资料失败：{error}"))?;
    }
    get(conn, id)
}

pub fn mark_processing(conn: &Connection, id: i64) -> AppResult<()> {
    conn.execute("UPDATE documents SET status = 'processing' WHERE id = ?1", [id])?;
    Ok(())
}

pub fn mark_failed(conn: &Connection, id: i64, error: &str) -> AppResult<()> {
    conn.execute(
        "UPDATE document_contents SET content = '', error = ?1 WHERE document_id = ?2",
        params![error, id],
    )?;
    conn.execute("UPDATE documents SET status = 'failed' WHERE id = ?1", [id])?;
    Ok(())
}

/// Re-parsing failed: keep the previous good result and only report the error.
pub fn restore_after_failed_reparse(conn: &Connection, id: i64) -> AppResult<()> {
    conn.execute("UPDATE documents SET status = 'completed' WHERE id = ?1", [id])?;
    Ok(())
}

/// Stores a successful extraction: text, units, chapters, chunks, index and figures.
pub fn apply_extraction(conn: &mut Connection, id: i64, extraction: &Extraction) -> AppResult<Document> {
    edit::validate_locked(conn, id, extraction)?;
    let filename: String = conn
        .query_row("SELECT filename FROM documents WHERE id = ?1", [id], |r| r.get(0))
        .or_not_found("资料")?;
    let tx = conn.transaction()?;
    index::store(&tx, id, &stem(&filename), extraction)?;
    tx.execute(
        "UPDATE document_contents SET content = ?1, error = NULL WHERE document_id = ?2",
        params![extraction.content, id],
    )?;
    tx.execute("UPDATE documents SET status = 'completed' WHERE id = ?1", [id])?;
    tx.commit()?;
    get(conn, id)
}

/// Runs the processor for a stored document and applies the result.
/// Returns the updated document; processing errors are recorded, not raised,
/// unless `keep_previous` is set (re-parse of a good document).
pub async fn process(
    paths: &crate::database::AppPaths,
    id: i64,
    keep_previous: bool,
    on_progress: impl FnMut(ParseProgress) + Send,
) -> AppResult<Document> {
    let document = {
        let conn = paths.connect()?;
        get(&conn, id)?
    };
    let final_dir = figures_dir(&paths.documents_dir, document.course_id, id);
    let staging = final_dir.with_extension("new");
    if staging.exists() {
        fs::remove_dir_all(&staging)?;
    }
    let source = locate_copy(&paths.documents_dir, &document);
    let result = parse::run(&source, &document.file_type, &staging, on_progress).await;
    let mut conn = paths.connect()?;
    match result {
        Ok(mut extraction) => {
            if let Err(error) = edit::validate_locked(&conn, id, &extraction) {
                let _ = fs::remove_dir_all(&staging);
                restore_after_failed_reparse(&conn, id)?;
                return Err(error);
            }
            // Swap in the new figures, then point records at their final location.
            if final_dir.exists() {
                fs::remove_dir_all(&final_dir)?;
            }
            if staging.exists() {
                fs::rename(&staging, &final_dir)?;
            }
            for image in &mut extraction.images {
                if let Some(name) = Path::new(&image.path).file_name() {
                    image.path = final_dir.join(name).to_string_lossy().into_owned();
                }
            }
            let preview_cache = paths.cache_dir.join("pages").join(id.to_string());
            if preview_cache.exists() {
                let _ = fs::remove_dir_all(preview_cache);
            }
            apply_extraction(&mut conn, id, &extraction)
        }
        Err(error) => {
            let _ = fs::remove_dir_all(&staging);
            if keep_previous {
                restore_after_failed_reparse(&conn, id)?;
                Err(AppError::msg(format!("重新解析失败，已保留原有结果：{error}")))
            } else {
                mark_failed(&conn, id, &error)?;
                get(&conn, id)
            }
        }
    }
}

/// Phase-one documents only have plain text; index them so search works at once.
pub fn backfill_legacy(conn: &mut Connection) -> AppResult<usize> {
    let pending: Vec<(i64, String, String)> = {
        let mut stmt = conn.prepare(
            "SELECT d.id, d.file_type, c.content FROM documents d JOIN document_contents c ON c.document_id = d.id
             WHERE d.status = 'completed' AND d.indexed_at IS NULL AND c.content != ''",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    for (id, file_type, content) in &pending {
        let extraction = Extraction {
            content: content.clone(),
            loc_kind: match file_type.as_str() {
                "pdf" => "page",
                "pptx" => "slide",
                "docx" => "paragraph",
                _ => "line",
            }
            .into(),
            structure_source: "legacy".into(),
            pages: parse::pages_from_plain_text(content, file_type),
            images: Vec::new(),
            warnings: vec!["由旧版本导入：重新解析后可识别章节和图片".into()],
        };
        apply_extraction(conn, *id, &extraction)?;
    }
    Ok(pending.len())
}

pub fn sections(conn: &Connection, document_id: i64) -> AppResult<Vec<Section>> {
    let loc_kind: String = conn
        .query_row("SELECT loc_kind FROM documents WHERE id = ?1", [document_id], |r| r.get(0))
        .or_not_found("资料")?;
    let mut stmt = conn.prepare(
        "SELECT s.id, s.document_id, s.parent_id, s.ordinal, s.level, s.title, s.page_start, s.page_end, s.char_count,
                (SELECT loc_start FROM document_pages p WHERE p.document_id = s.document_id AND p.page_number = s.page_start),
                (SELECT loc_end FROM document_pages p WHERE p.document_id = s.document_id AND p.page_number = s.page_end),
                s.block_start, s.block_end, s.source, s.user_edited, s.locked
         FROM document_sections s WHERE s.document_id = ?1 ORDER BY s.ordinal",
    )?;
    let rows = stmt.query_map([document_id], |r| {
        let page_start: i64 = r.get(6)?;
        let page_end: i64 = r.get(7)?;
        let loc_start: Option<i64> = r.get(9)?;
        let loc_end: Option<i64> = r.get(10)?;
        let (a, b) = if loc_kind == "line" {
            (loc_start.unwrap_or(page_start), loc_end.unwrap_or(page_end))
        } else {
            (page_start, page_end)
        };
        Ok(Section {
            id: r.get(0)?,
            document_id: r.get(1)?,
            parent_id: r.get(2)?,
            ordinal: r.get(3)?,
            level: r.get(4)?,
            title: r.get(5)?,
            page_start,
            page_end,
            char_count: r.get(8)?,
            loc_label: index::loc_label(&loc_kind, a, b),
            block_start: r.get(11)?,
            block_end: r.get(12)?,
            source: r.get(13)?,
            user_edited: r.get(14)?,
            locked: r.get(15)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn pages(conn: &Connection, document_id: i64, start: i64, end: i64) -> AppResult<Vec<PageView>> {
    if end < start || end - start > 400 {
        return Err(AppError::invalid("一次最多读取 400 个单元"));
    }
    let mut images: std::collections::HashMap<i64, Vec<ImageInfo>> = std::collections::HashMap::new();
    {
        let mut stmt = conn.prepare(
            "SELECT id, page_number, ordinal, width, height, ocr_text FROM document_images
             WHERE document_id = ?1 AND page_number BETWEEN ?2 AND ?3 ORDER BY page_number, ordinal",
        )?;
        let rows = stmt.query_map(params![document_id, start, end], |r| {
            Ok(ImageInfo {
                id: r.get(0)?,
                page_number: r.get(1)?,
                ordinal: r.get(2)?,
                width: r.get(3)?,
                height: r.get(4)?,
                ocr_text: r.get(5)?,
            })
        })?;
        for image in rows {
            let image = image?;
            images.entry(image.page_number).or_default().push(image);
        }
    }
    let mut stmt = conn.prepare(
        "SELECT page_number, label, loc_start, loc_end, blocks, has_ocr, width, height FROM document_pages
         WHERE document_id = ?1 AND page_number BETWEEN ?2 AND ?3 ORDER BY page_number",
    )?;
    let rows = stmt.query_map(params![document_id, start, end], |r| {
        let number: i64 = r.get(0)?;
        let blocks: String = r.get(4)?;
        Ok(PageView {
            page_number: number,
            label: r.get(1)?,
            loc_start: r.get(2)?,
            loc_end: r.get(3)?,
            blocks: serde_json::from_str(&blocks).unwrap_or_default(),
            has_ocr: r.get(5)?,
            width: r.get(6)?,
            height: r.get(7)?,
            images: Vec::new(),
        })
    })?;
    let mut result = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    for page in &mut result {
        page.images = images.remove(&page.page_number).unwrap_or_default();
    }
    Ok(result)
}

/// An extracted figure; like [`locate_copy`], survives a relocated data folder.
pub fn image_path(conn: &Connection, root: &Path, image_id: i64) -> AppResult<PathBuf> {
    let (stored, document_id, course_id): (String, i64, i64) = conn
        .query_row(
            "SELECT i.file_path, i.document_id, d.course_id
            FROM document_images i JOIN documents d ON d.id = i.document_id WHERE i.id = ?1",
            [image_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .or_not_found("图片")?;
    let stored = PathBuf::from(stored);
    if !stored.is_file() {
        // Stored paths use the separators of the computer that wrote them.
        let name = stored.to_string_lossy().rsplit(['/', '\\']).next().unwrap_or_default().to_string();
        let derived = figures_dir(root, course_id, document_id).join(name);
        if derived.is_file() {
            return Ok(derived);
        }
    }
    Ok(stored)
}

pub fn delete_document(conn: &mut Connection, paths: &crate::database::AppPaths, id: i64) -> AppResult<()> {
    let document = get(conn, id)?;
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM documents WHERE id = ?1", [id])?;
    tx.commit()?;
    // Remove only application-owned copies; the user's original file is untouched.
    let copy = locate_copy(&paths.documents_dir, &document);
    if copy.starts_with(&paths.documents_dir) && copy.exists() {
        fs::remove_file(&copy)?;
    }
    let figures = figures_dir(&paths.documents_dir, document.course_id, id);
    if figures.exists() {
        fs::remove_dir_all(figures)?;
    }
    let previews = paths.cache_dir.join("pages").join(id.to_string());
    if previews.exists() {
        let _ = fs::remove_dir_all(previews);
    }
    Ok(())
}

pub fn delete_course(conn: &mut Connection, paths: &crate::database::AppPaths, id: i64) -> AppResult<()> {
    let document_ids: Vec<i64> = {
        let mut stmt = conn.prepare("SELECT id FROM documents WHERE course_id = ?1")?;
        let rows = stmt.query_map([id], |r| r.get(0))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    let tx = conn.transaction()?;
    let count = tx.execute("DELETE FROM courses WHERE id = ?1", [id])?;
    if count == 0 {
        return Err(AppError::NotFound("课程"));
    }
    // Paths are derived from application-owned storage, never from source filenames.
    let directory = paths.documents_dir.join(id.to_string());
    if directory.exists() {
        fs::remove_dir_all(directory).map_err(|e| AppError::msg(format!("删除资料副本失败：{e}")))?;
    }
    tx.commit()?;
    for document_id in document_ids {
        let _ = fs::remove_dir_all(paths.cache_dir.join("pages").join(document_id.to_string()));
    }
    Ok(())
}

/// Returns the cached PNG for a PDF page, rendering it with the processor if needed.
pub async fn page_preview(
    paths: &crate::database::AppPaths,
    document_id: i64,
    page: i64,
    width: u32,
) -> AppResult<PathBuf> {
    let width = width.clamp(400, 2000) / 100 * 100;
    let document = get(&paths.connect()?, document_id)?;
    if document.file_type != "pdf" {
        return Err(AppError::invalid("只有 PDF 支持原页面预览"));
    }
    if page < 1 || page > document.page_count.max(1) {
        return Err(AppError::invalid("页码超出范围"));
    }
    let dir = paths.cache_dir.join("pages").join(document_id.to_string());
    let target = dir.join(format!("p{page}_w{width}.png"));
    if !target.exists() {
        parse::render_pages(&locate_copy(&paths.documents_dir, &document), &[page], &dir, width).await?;
    }
    if !target.exists() {
        return Err(AppError::msg("页面预览生成失败"));
    }
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{self, sample_extraction, temp_paths};
    use parse::ParsedImage;

    #[test]
    fn imports_are_independent_persist_and_delete_only_owned_copies() {
        let paths = temp_paths("import");
        let source = paths.data_dir.join("原始资料.txt");
        fs::write(&source, "学习资料").unwrap();
        let mut conn = paths.connect().unwrap();
        conn.execute("INSERT INTO courses (name) VALUES ('课程')", []).unwrap();
        let first = prepare(&mut conn, &paths.documents_dir, 1, &source).unwrap();
        let second = prepare(&mut conn, &paths.documents_dir, 1, &source).unwrap();
        assert_ne!(first.file_path, second.file_path);
        apply_extraction(&mut conn, first.id, &sample_extraction()).unwrap();
        mark_failed(&conn, second.id, "无法识别").unwrap();
        drop(conn);
        let mut conn = paths.connect().unwrap();
        assert!(body(&conn, first.id).unwrap().contains("函数"));
        assert_eq!(get(&conn, second.id).unwrap().error.as_deref(), Some("无法识别"));
        assert!(prepare(&mut conn, &paths.documents_dir, 999, &source).is_err());
        delete_course(&mut conn, &paths, 1).unwrap();
        assert!(list(&conn, 1).unwrap().is_empty());
        assert!(!Path::new(&first.file_path).exists());
        assert_eq!(fs::read_to_string(&source).unwrap(), "学习资料");
        // FTS rows are removed together with the chunks (cascade + trigger).
        let fts: i64 = conn.query_row("SELECT COUNT(*) FROM document_chunks_fts", [], |r| r.get(0)).unwrap();
        assert_eq!(fts, 0);
        drop(conn);
        test_support::cleanup(paths);
    }

    #[test]
    fn unsupported_and_empty_files_are_rejected() {
        let paths = temp_paths("reject");
        let mut conn = paths.connect().unwrap();
        conn.execute("INSERT INTO courses (name) VALUES ('课程')", []).unwrap();
        let doc = paths.data_dir.join("旧版.doc");
        fs::write(&doc, "x").unwrap();
        assert!(prepare(&mut conn, &paths.documents_dir, 1, &doc).unwrap_err().to_string().contains("支持"));
        let empty = paths.data_dir.join("空.txt");
        fs::write(&empty, "").unwrap();
        assert!(prepare(&mut conn, &paths.documents_dir, 1, &empty).unwrap_err().to_string().contains("为空"));
        drop(conn);
        test_support::cleanup(paths);
    }

    #[test]
    fn extraction_is_stored_with_sections_pages_images_and_reparse_replaces_it() {
        let (paths, mut conn) = test_support::course_with_document();
        let mut extraction = sample_extraction();
        extraction.images = vec![ParsedImage { page: 3, ordinal: 1, path: "/tmp/x.png".into(), width: 200, height: 100, ocr_text: String::new() }];
        let stored = apply_extraction(&mut conn, 1, &extraction).unwrap();
        assert_eq!((stored.status.as_str(), stored.page_count, stored.section_count, stored.image_count), ("completed", 3, 3, 1));
        let tree = sections(&conn, 1).unwrap();
        assert_eq!(tree.iter().map(|s| (s.level, s.title.as_str())).collect::<Vec<_>>(),
                   vec![(1, "第一章 函数与极限"), (2, "1.1 极限的定义"), (1, "第二章 导数")]);
        assert_eq!(tree[0].loc_label, "第 1–2 页");
        assert_eq!(tree[1].parent_id, Some(tree[0].id));
        let views = pages(&conn, 1, 1, 3).unwrap();
        assert_eq!(views[2].images.len(), 1);
        assert_eq!(views[0].blocks[0].heading, Some(1));
        // Re-parse replaces rows instead of duplicating them.
        apply_extraction(&mut conn, 1, &sample_extraction()).unwrap();
        let again = get(&conn, 1).unwrap();
        assert_eq!((again.section_count, again.image_count), (3, 0));
        let chunks: i64 = conn.query_row("SELECT COUNT(*) FROM document_chunks WHERE document_id = 1", [], |r| r.get(0)).unwrap();
        let fts: i64 = conn.query_row("SELECT COUNT(*) FROM document_chunks_fts", [], |r| r.get(0)).unwrap();
        assert_eq!(chunks, fts);
        delete_document(&mut conn, &paths, 1).unwrap();
        assert!(get(&conn, 1).is_err());
        drop(conn);
        test_support::cleanup(paths);
    }

    #[test]
    fn formula_warnings_and_source_text_survive_storage_and_retrieval() {
        let (paths, mut conn) = test_support::course_with_document();
        let mut extraction = sample_extraction();
        let formula = ParsedBlock { text: "公式 𝑥²≠α".into(), raw_text: Some("公式 𝑥²≠α".into()),
            formula: true, recognition: "unverified".into(), bbox: Some([1., 2., 30., 40.]),
            kind: "ocr".into(), ..Default::default() };
        extraction.pages[0].blocks.push(formula.clone());
        apply_extraction(&mut conn, 1, &extraction).unwrap();
        assert_eq!(pages(&conn, 1, 1, 1).unwrap()[0].blocks.last(), Some(&formula));
        let hits = crate::context::scope_chunks(&conn, 1, &Default::default()).unwrap();
        assert_eq!(hits.iter().filter(|c| c.unreliable_formula).count(), 1);
        assert!(crate::context::render(&hits, 1).0.contains("识别警告"));
        let retrieved = crate::search::retrieve(&conn, 1, "公式", crate::search::segment::QueryMode::All, &Default::default(), 5).unwrap();
        assert!(retrieved[0].unreliable_formula);
        drop(conn);
        test_support::cleanup(paths);
    }

    #[test]
    fn corrected_same_page_titles_keep_existing_note_references() {
        let (paths, mut conn) = test_support::course_with_document();
        let mut extraction = sample_extraction();
        extraction.pages.truncate(1);
        extraction.pages[0].blocks.extend([
            ParsedBlock { text: "第二章旧识别".into(), heading: Some(1), ..Default::default() },
            ParsedBlock { text: "第二章正文".into(), ..Default::default() },
        ]);
        apply_extraction(&mut conn, 1, &extraction).unwrap();
        let old = sections(&conn, 1).unwrap()[1].id;
        conn.execute("INSERT INTO notes (course_id, title, content, document_id, section_id) VALUES (1, '笔记', '保留', 1, ?1)", [old]).unwrap();
        extraction.pages[0].blocks[2].text = "第二章正确标题".into();
        apply_extraction(&mut conn, 1, &extraction).unwrap();
        let title: String = conn.query_row("SELECT s.title FROM notes n JOIN document_sections s ON n.section_id = s.id", [], |r| r.get(0)).unwrap();
        assert_eq!(title, "第二章正确标题");
        drop(conn);
        test_support::cleanup(paths);
    }

    #[test]
    fn a_data_folder_restored_elsewhere_still_finds_copies_and_figures() {
        let paths = temp_paths("relocate-old");
        let source = paths.data_dir.join("讲义.txt");
        fs::write(&source, "学习资料").unwrap();
        let mut conn = paths.connect().unwrap();
        conn.execute("INSERT INTO courses (name) VALUES ('课程')", []).unwrap();
        let document = prepare(&mut conn, &paths.documents_dir, 1, &source).unwrap();
        let figures = figures_dir(&paths.documents_dir, 1, document.id);
        fs::create_dir_all(&figures).unwrap();
        fs::write(figures.join("p1_1.png"), b"png").unwrap();
        let mut extraction = sample_extraction();
        let figure = figures.join("p1_1.png").to_string_lossy().into_owned();
        extraction.images = vec![ParsedImage { page: 1, ordinal: 1, path: figure, width: 10, height: 10, ocr_text: String::new() }];
        apply_extraction(&mut conn, document.id, &extraction).unwrap();
        let image_id: i64 = conn.query_row("SELECT id FROM document_images", [], |r| r.get(0)).unwrap();
        drop(conn);

        // Backup copied to another computer / Windows user: same files, different absolute root.
        let restored = temp_paths("relocate-new");
        fs::remove_dir_all(&restored.data_dir).unwrap();
        fs::rename(&paths.data_dir, &restored.data_dir).unwrap();
        let mut conn = restored.connect().unwrap();
        let moved = get(&conn, document.id).unwrap();
        assert!(!Path::new(&moved.file_path).exists());
        let copy = locate_copy(&restored.documents_dir, &moved);
        assert!(copy.starts_with(&restored.documents_dir));
        assert_eq!(fs::read_to_string(&copy).unwrap(), "学习资料");
        let image = image_path(&conn, &restored.documents_dir, image_id).unwrap();
        assert_eq!(fs::read(&image).unwrap(), b"png");
        // Deleting still removes the application's copy at its new location.
        delete_document(&mut conn, &restored, document.id).unwrap();
        assert!(!copy.exists() && !figures_dir(&restored.documents_dir, 1, document.id).exists());
        drop(conn);
        test_support::cleanup(restored);
    }

    #[test]
    fn legacy_documents_are_backfilled() {
        let paths = temp_paths("legacy");
        let mut conn = paths.connect().unwrap();
        conn.execute("INSERT INTO courses (name) VALUES ('旧课程')", []).unwrap();
        conn.execute(
            "INSERT INTO documents (course_id, filename, file_type, file_path, file_size, status) VALUES (1, '旧.pdf', 'pdf', '', 1, 'completed')",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO document_contents (document_id, content) VALUES (1, '[第 1 页]\n导数的定义\n\n[第 2 页]\n积分')", [])
            .unwrap();
        assert_eq!(backfill_legacy(&mut conn).unwrap(), 1);
        let doc = get(&conn, 1).unwrap();
        assert_eq!((doc.page_count, doc.structure_source.as_str()), (2, "legacy"));
        assert_eq!(backfill_legacy(&mut conn).unwrap(), 0);
        drop(conn);
        test_support::cleanup(paths);
    }
}
