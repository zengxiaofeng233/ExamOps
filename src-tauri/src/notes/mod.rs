//! Study notes, optionally linked to a document page, chapter or knowledge point.

use crate::error::{AppError, AppResult, OptionalRow};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct Note {
    pub id: i64,
    pub course_id: i64,
    pub title: String,
    pub content: String,
    pub document_id: Option<i64>,
    pub document_name: Option<String>,
    pub page_number: Option<i64>,
    pub page_reference: Option<String>,
    pub section_id: Option<i64>,
    pub section_title: Option<String>,
    pub knowledge_point_id: Option<i64>,
    pub knowledge_point_title: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NoteInput {
    pub id: Option<i64>,
    pub course_id: i64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub document_id: Option<i64>,
    #[serde(default)]
    pub page_number: Option<i64>,
    #[serde(default)]
    pub section_id: Option<i64>,
    #[serde(default)]
    pub knowledge_point_id: Option<i64>,
}

const SELECT: &str = "SELECT n.id, n.course_id, n.title, n.content, n.document_id, d.filename, n.page_number, n.page_reference,
    n.section_id, s.title, n.knowledge_point_id, k.title, n.created_at, n.updated_at
    FROM notes n
    LEFT JOIN documents d ON d.id = n.document_id
    LEFT JOIN document_sections s ON s.id = n.section_id
    LEFT JOIN knowledge_points k ON k.id = n.knowledge_point_id";

fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Note> {
    Ok(Note {
        id: row.get(0)?,
        course_id: row.get(1)?,
        title: row.get(2)?,
        content: row.get(3)?,
        document_id: row.get(4)?,
        document_name: row.get(5)?,
        page_number: row.get(6)?,
        page_reference: row.get(7)?,
        section_id: row.get(8)?,
        section_title: row.get(9)?,
        knowledge_point_id: row.get(10)?,
        knowledge_point_title: row.get(11)?,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
    })
}

pub fn list(conn: &Connection, course_id: i64) -> AppResult<Vec<Note>> {
    let mut stmt = conn.prepare(&format!("{SELECT} WHERE n.course_id = ?1 ORDER BY n.updated_at DESC, n.id DESC"))?;
    let rows = stmt.query_map([course_id], from_row)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn get(conn: &Connection, id: i64) -> AppResult<Note> {
    conn.query_row(&format!("{SELECT} WHERE n.id = ?1"), [id], from_row).or_not_found("笔记")
}

fn belongs(conn: &Connection, sql: &str, id: Option<i64>, course_id: i64, what: &'static str) -> AppResult<()> {
    if let Some(id) = id {
        let course: i64 = conn.query_row(sql, [id], |r| r.get(0)).or_not_found(what)?;
        if course != course_id {
            return Err(AppError::invalid(format!("{what}不属于该课程")));
        }
    }
    Ok(())
}

pub fn save(conn: &Connection, input: NoteInput) -> AppResult<Note> {
    let content = input.content.trim();
    let mut title: String = input.title.trim().chars().take(80).collect();
    if title.is_empty() && content.is_empty() {
        return Err(AppError::invalid("笔记内容不能为空"));
    }
    if title.is_empty() {
        title = content.lines().next().unwrap_or("").chars().take(30).collect();
    }
    belongs(conn, "SELECT course_id FROM documents WHERE id = ?1", input.document_id, input.course_id, "资料")?;
    belongs(conn, "SELECT d.course_id FROM document_sections s JOIN documents d ON d.id = s.document_id WHERE s.id = ?1", input.section_id, input.course_id, "章节")?;
    belongs(conn, "SELECT course_id FROM knowledge_points WHERE id = ?1", input.knowledge_point_id, input.course_id, "知识点")?;
    let content: String = content.chars().take(100_000).collect();
    match input.id {
        Some(id) => {
            conn.execute(
                "UPDATE notes SET title = ?1, content = ?2, document_id = ?3, page_number = ?4, section_id = ?5, knowledge_point_id = ?6,
                 updated_at = datetime('now', 'localtime') WHERE id = ?7 AND course_id = ?8",
                params![title, content, input.document_id, input.page_number, input.section_id, input.knowledge_point_id, id, input.course_id],
            )?;
            get(conn, id)
        }
        None => {
            conn.execute(
                "INSERT INTO notes (course_id, title, content, document_id, page_number, section_id, knowledge_point_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![input.course_id, title, content, input.document_id, input.page_number, input.section_id, input.knowledge_point_id],
            )?;
            get(conn, conn.last_insert_rowid())
        }
    }
}

pub fn delete(conn: &Connection, id: i64) -> AppResult<()> {
    if conn.execute("DELETE FROM notes WHERE id = ?1", [id])? == 0 {
        return Err(AppError::NotFound("笔记"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support;

    #[test]
    fn notes_link_to_pages_and_validate_ownership() {
        let (paths, conn) = test_support::course_with_document();
        let note = save(&conn, NoteInput { id: None, course_id: 1, title: String::new(), content: "切线斜率就是导数\n更多内容".into(), document_id: Some(1), page_number: Some(3), section_id: None, knowledge_point_id: None }).unwrap();
        assert_eq!(note.title, "切线斜率就是导数");
        assert_eq!(note.document_name.as_deref(), Some("高等数学讲义.pdf"));
        assert!(save(&conn, NoteInput { id: None, course_id: 2, title: "x".into(), content: String::new(), document_id: Some(1), page_number: None, section_id: None, knowledge_point_id: None }).is_err());
        assert!(save(&conn, NoteInput { id: None, course_id: 1, title: " ".into(), content: " ".into(), document_id: None, page_number: None, section_id: None, knowledge_point_id: None }).is_err());
        let edited = save(&conn, NoteInput { id: Some(note.id), course_id: 1, title: "导数".into(), content: "新内容".into(), document_id: None, page_number: None, section_id: None, knowledge_point_id: None }).unwrap();
        assert_eq!((edited.title.as_str(), edited.document_id), ("导数", None));
        assert_eq!(list(&conn, 1).unwrap().len(), 1);
        delete(&conn, note.id).unwrap();
        assert!(list(&conn, 1).unwrap().is_empty());
        drop(conn);
        test_support::cleanup(paths);
    }
}
