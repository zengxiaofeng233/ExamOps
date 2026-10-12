use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Default)]
pub struct DocumentTasks(pub tokio::sync::Mutex<()>);

#[derive(Debug, Serialize)]
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
}

const SELECT: &str = "SELECT d.id, d.course_id, d.filename, d.file_type, d.file_path,
    d.file_size, d.status, d.created_at, c.error FROM documents d
    LEFT JOIN document_contents c ON c.document_id = d.id";

fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Document> {
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
    })
}

pub fn list(conn: &Connection, course_id: i64) -> Result<Vec<Document>, String> {
    let mut stmt = conn
        .prepare(&format!(
            "{SELECT} WHERE d.course_id = ?1 ORDER BY d.id DESC"
        ))
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([course_id], from_row)
        .map_err(|e| e.to_string())?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())
}

pub fn get(conn: &Connection, id: i64) -> Result<Document, String> {
    conn.query_row(&format!("{SELECT} WHERE d.id = ?1"), [id], from_row)
        .map_err(|e| e.to_string())
}

pub fn body(conn: &Connection, id: i64) -> Result<String, String> {
    conn.query_row(
        "SELECT content FROM document_contents WHERE document_id = ?1",
        [id],
        |r| r.get(0),
    )
    .map_err(|e| e.to_string())
}

pub fn prepare(
    conn: &mut Connection,
    root: &Path,
    course_id: i64,
    source: &Path,
) -> Result<Document, String> {
    let file_type = source
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    if !["pdf", "docx", "pptx", "txt", "md", "markdown"].contains(&file_type.as_str()) {
        return Err("支持 PDF、DOCX、PPTX、TXT 和 Markdown 文件".into());
    }
    let metadata = source.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file() {
        return Err("请选择文件".into());
    }
    let filename = source
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("文件名无效")?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO documents (course_id, filename, file_type, file_path, file_size, status)
        VALUES (?1, ?2, ?3, '', ?4, 'processing')",
        params![course_id, filename, file_type, metadata.len() as i64],
    )
    .map_err(|e| e.to_string())?;
    let id = tx.last_insert_rowid();
    let target = root
        .join(course_id.to_string())
        .join(format!("{id}.{file_type}"));
    tx.execute(
        "UPDATE documents SET file_path = ?1 WHERE id = ?2",
        params![target.to_string_lossy(), id],
    )
    .map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO document_contents (document_id) VALUES (?1)",
        [id],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    let copied = (|| {
        fs::create_dir_all(target.parent().unwrap())?;
        fs::copy(source, &target)?;
        Ok::<(), std::io::Error>(())
    })();
    if let Err(error) = copied {
        finish(conn, id, Err(format!("复制资料失败：{error}")))?;
    }
    get(conn, id)
}

pub fn finish(
    conn: &mut Connection,
    id: i64,
    result: Result<String, String>,
) -> Result<Document, String> {
    let (status, content, error) = match result {
        Ok(body) => ("completed", body, None),
        Err(error) => ("failed", String::new(), Some(error)),
    };
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute(
        "UPDATE document_contents SET content = ?1, error = ?2 WHERE document_id = ?3",
        params![content, error, id],
    )
    .map_err(|e| e.to_string())?;
    tx.execute(
        "UPDATE documents SET status = ?1 WHERE id = ?2",
        params![status, id],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    get(conn, id)
}

pub fn delete_course(conn: &mut Connection, root: &Path, id: i64) -> Result<(), String> {
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM document_chunks_fts WHERE document_id IN (SELECT id FROM documents WHERE course_id = ?1)", [id])
        .map_err(|e| e.to_string())?;
    let count = tx
        .execute("DELETE FROM courses WHERE id = ?1", [id])
        .map_err(|e| e.to_string())?;
    if count == 0 {
        return Err("Course not found".into());
    }
    // Paths are derived from application-owned storage, never from source filenames.
    let directory = root.join(id.to_string());
    if directory.exists() {
        fs::remove_dir_all(directory).map_err(|e| format!("删除资料副本失败：{e}"))?;
    }
    tx.commit().map_err(|e| e.to_string())
}

#[derive(Deserialize)]
struct ParserResponse {
    status: String,
    content: Option<String>,
    message: Option<String>,
}

pub async fn extract(path: &Path) -> Result<String, String> {
    let executable = if cfg!(debug_assertions) {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("binaries/examops-parser-x86_64-pc-windows-msvc.exe")
    } else {
        std::env::current_exe()
            .map_err(|e| e.to_string())?
            .with_file_name("examops-parser.exe")
    };
    let mut command = tokio::process::Command::new(executable);
    command
        .args([std::ffi::OsStr::new("--input"), path.as_os_str()])
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    let output = command
        .output()
        .await
        .map_err(|e| format!("无法启动文档处理程序：{e}"))?;
    if !output.status.success() {
        return Err(format!(
            "文档处理程序退出异常：{}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let response: ParserResponse = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("文档处理程序返回无效结果：{e}"))?;
    if response.status == "success" {
        response
            .content
            .filter(|s| !s.trim().is_empty())
            .ok_or("未提取到正文".into())
    } else {
        Err(response.message.unwrap_or_else(|| "正文提取失败".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imports_are_independent_persist_and_delete_only_owned_copies() {
        let folder = std::env::temp_dir().join(format!(
            "examops-test-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir_all(&folder).unwrap();
        let source = folder.join("原始资料.txt");
        fs::write(&source, "学习资料").unwrap();
        let db = folder.join("test.db");
        let root = folder.join("documents");
        let mut conn = crate::database::open_connection(&db).unwrap();
        crate::database::schema::create_tables(&conn).unwrap();
        conn.execute("INSERT INTO courses (name) VALUES ('课程')", [])
            .unwrap();
        let first = prepare(&mut conn, &root, 1, &source).unwrap();
        let second = prepare(&mut conn, &root, 1, &source).unwrap();
        assert_ne!(first.file_path, second.file_path);
        finish(&mut conn, first.id, Ok("学习资料".into())).unwrap();
        finish(&mut conn, second.id, Err("无法识别".into())).unwrap();
        drop(conn);
        let mut conn = crate::database::open_connection(&db).unwrap();
        assert_eq!(body(&conn, first.id).unwrap(), "学习资料");
        assert_eq!(
            get(&conn, second.id).unwrap().error.as_deref(),
            Some("无法识别")
        );
        assert!(prepare(&mut conn, &root, 999, &source).is_err());
        delete_course(&mut conn, &root, 1).unwrap();
        assert!(list(&conn, 1).unwrap().is_empty());
        assert!(!Path::new(&first.file_path).exists());
        assert_eq!(fs::read_to_string(&source).unwrap(), "学习资料");
        drop(conn);
        fs::remove_dir_all(folder).unwrap();
    }
}
