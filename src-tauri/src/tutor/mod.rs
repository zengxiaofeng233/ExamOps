//! Tutor Engine: conversations with the AI about the student's own materials.

pub mod chat;
pub mod prompts;
pub mod teach;

use crate::context::{ContextMode, ConversationScope, SourceRef};
use crate::error::{AppError, AppResult, OptionalRow};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MODES: [&str; 4] = ["qa", "learn", "review", "sprint"];

/// Progress of a "从零学习" conversation over the lesson parts of a chapter
/// (or of a video transcript). See `teach::lesson_parts`.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct TeachProgress {
    /// Index of the next part to teach.
    #[serde(default)]
    pub next: usize,
    /// Number of parts when progress was last saved.
    #[serde(default)]
    pub parts: usize,
    /// Part taught most recently (context for examples, re-explanations and questions).
    #[serde(default)]
    pub last: Option<usize>,
    #[serde(default)]
    pub taught: Vec<usize>,
    #[serde(default)]
    pub skipped: Vec<usize>,
    #[serde(default)]
    pub finished: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Conversation {
    pub id: i64,
    pub course_id: i64,
    pub mode: String,
    pub title: String,
    pub context_mode: ContextMode,
    pub scope: ConversationScope,
    pub profile_id: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
    pub message_count: i64,
    pub last_message: Option<String>,
    pub section_id: Option<i64>,
    pub section_title: Option<String>,
    pub step_id: Option<i64>,
    pub resource_id: Option<i64>,
    pub resource_title: Option<String>,
    pub progress: TeachProgress,
}

#[derive(Debug, Clone, Serialize)]
pub struct Message {
    pub id: i64,
    pub conversation_id: i64,
    pub role: String,
    pub content: String,
    pub status: String,
    pub error: Option<String>,
    pub sources: Vec<SourceRef>,
    pub meta: Value,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConversationDetail {
    pub conversation: Conversation,
    pub messages: Vec<Message>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateConversation {
    pub course_id: i64,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub context_mode: Option<ContextMode>,
    #[serde(default)]
    pub scope: Option<ConversationScope>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateConversation {
    pub id: i64,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub context_mode: Option<ContextMode>,
    #[serde(default)]
    pub scope: Option<ConversationScope>,
    /// `Some(0)` returns to the default model.
    #[serde(default)]
    pub profile_id: Option<i64>,
}

const CONVERSATION_COLUMNS: &str = "c.id, c.course_id, c.mode, c.title, c.context_mode, c.scope, c.profile_id, c.created_at, c.updated_at,
    (SELECT COUNT(*) FROM messages m WHERE m.conversation_id = c.id),
    (SELECT substr(m.content, 1, 80) FROM messages m WHERE m.conversation_id = c.id AND m.content != '' ORDER BY m.id DESC LIMIT 1),
    c.section_id, s.title, c.step_id, c.resource_id, r.title, c.progress";

const CONVERSATION_FROM: &str = "conversations c LEFT JOIN document_sections s ON s.id = c.section_id LEFT JOIN resources r ON r.id = c.resource_id";

fn conversation_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Conversation> {
    let mode: String = row.get(4)?;
    let scope: String = row.get(5)?;
    Ok(Conversation {
        id: row.get(0)?,
        course_id: row.get(1)?,
        mode: row.get(2)?,
        title: row.get(3)?,
        context_mode: ContextMode::parse(&mode),
        scope: serde_json::from_str(&scope).unwrap_or_default(),
        profile_id: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
        message_count: row.get(9)?,
        last_message: row.get(10)?,
        section_id: row.get(11)?,
        section_title: row.get(12)?,
        step_id: row.get(13)?,
        resource_id: row.get(14)?,
        resource_title: row.get(15)?,
        progress: serde_json::from_str(&row.get::<_, String>(16)?).unwrap_or_default(),
    })
}

const MESSAGE_COLUMNS: &str = "id, conversation_id, role, content, status, error, sources, meta, created_at";

fn message_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Message> {
    let sources: String = row.get(6)?;
    let meta: String = row.get(7)?;
    Ok(Message {
        id: row.get(0)?,
        conversation_id: row.get(1)?,
        role: row.get(2)?,
        content: row.get(3)?,
        status: row.get(4)?,
        error: row.get(5)?,
        sources: serde_json::from_str(&sources).unwrap_or_default(),
        meta: serde_json::from_str(&meta).unwrap_or(Value::Null),
        created_at: row.get(8)?,
    })
}

fn mode_name(mode: ContextMode) -> &'static str {
    match mode {
        ContextMode::Auto => "auto",
        ContextMode::Full => "full",
        ContextMode::Rag => "rag",
    }
}

pub fn list(conn: &Connection, course_id: i64) -> AppResult<Vec<Conversation>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {CONVERSATION_COLUMNS} FROM {CONVERSATION_FROM} WHERE c.course_id = ?1 ORDER BY c.updated_at DESC, c.id DESC"
    ))?;
    let rows = stmt.query_map([course_id], conversation_from_row)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn get(conn: &Connection, id: i64) -> AppResult<Conversation> {
    conn.query_row(&format!("SELECT {CONVERSATION_COLUMNS} FROM {CONVERSATION_FROM} WHERE c.id = ?1"), [id], conversation_from_row)
        .or_not_found("对话")
}

pub fn messages(conn: &Connection, conversation_id: i64) -> AppResult<Vec<Message>> {
    let mut stmt = conn.prepare(&format!("SELECT {MESSAGE_COLUMNS} FROM messages WHERE conversation_id = ?1 ORDER BY id"))?;
    let rows = stmt.query_map([conversation_id], message_from_row)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn get_message(conn: &Connection, id: i64) -> AppResult<Message> {
    conn.query_row(&format!("SELECT {MESSAGE_COLUMNS} FROM messages WHERE id = ?1"), [id], message_from_row)
        .or_not_found("消息")
}

pub fn detail(conn: &Connection, id: i64) -> AppResult<ConversationDetail> {
    Ok(ConversationDetail { conversation: get(conn, id)?, messages: messages(conn, id)? })
}

pub fn create(conn: &Connection, input: CreateConversation, default_mode: ContextMode) -> AppResult<Conversation> {
    let mode = input.mode.unwrap_or_else(|| "qa".into());
    if !MODES.contains(&mode.as_str()) {
        return Err(AppError::invalid("未知的学习模式"));
    }
    let exists: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM courses WHERE id = ?1)", [input.course_id], |r| r.get(0))?;
    if !exists {
        return Err(AppError::NotFound("课程"));
    }
    conn.execute(
        "INSERT INTO conversations (course_id, mode, title, context_mode, scope) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            input.course_id,
            mode,
            input.title.unwrap_or_default().trim(),
            mode_name(input.context_mode.unwrap_or(default_mode)),
            serde_json::to_string(&input.scope.unwrap_or_default())?
        ],
    )?;
    get(conn, conn.last_insert_rowid())
}

pub fn update(conn: &Connection, input: UpdateConversation) -> AppResult<Conversation> {
    get(conn, input.id)?;
    if let Some(title) = input.title {
        conn.execute("UPDATE conversations SET title = ?1 WHERE id = ?2", params![title.trim().chars().take(80).collect::<String>(), input.id])?;
    }
    if let Some(mode) = input.context_mode {
        conn.execute("UPDATE conversations SET context_mode = ?1 WHERE id = ?2", params![mode_name(mode), input.id])?;
    }
    if let Some(scope) = input.scope {
        conn.execute("UPDATE conversations SET scope = ?1 WHERE id = ?2", params![serde_json::to_string(&scope)?, input.id])?;
    }
    if let Some(profile) = input.profile_id {
        let profile = if profile <= 0 { None } else { Some(profile) };
        conn.execute("UPDATE conversations SET profile_id = ?1 WHERE id = ?2", params![profile, input.id])?;
    }
    get(conn, input.id)
}

pub fn save_progress(conn: &Connection, id: i64, progress: &TeachProgress) -> AppResult<()> {
    conn.execute("UPDATE conversations SET progress = ?1 WHERE id = ?2", params![serde_json::to_string(progress)?, id])?;
    Ok(())
}

pub fn delete(conn: &Connection, id: i64) -> AppResult<()> {
    if conn.execute("DELETE FROM conversations WHERE id = ?1", [id])? == 0 {
        return Err(AppError::NotFound("对话"));
    }
    Ok(())
}

pub fn insert_message(conn: &Connection, conversation_id: i64, role: &str, content: &str, status: &str, sources: &[SourceRef], meta: &Value) -> AppResult<Message> {
    conn.execute(
        "INSERT INTO messages (conversation_id, role, content, status, sources, meta) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![conversation_id, role, content, status, serde_json::to_string(sources)?, meta.to_string()],
    )?;
    get_message(conn, conn.last_insert_rowid())
}

pub fn save_partial(conn: &Connection, id: i64, content: &str) -> AppResult<()> {
    conn.execute("UPDATE messages SET content = ?1 WHERE id = ?2", params![content, id])?;
    Ok(())
}

pub fn finish_message(conn: &Connection, id: i64, content: &str, status: &str, error: Option<&str>, sources: &[SourceRef], meta: &Value) -> AppResult<Message> {
    conn.execute(
        "UPDATE messages SET content = ?1, status = ?2, error = ?3, sources = ?4, meta = ?5 WHERE id = ?6",
        params![content, status, error, serde_json::to_string(sources)?, meta.to_string(), id],
    )?;
    get_message(conn, id)
}

pub fn touch(conn: &Connection, conversation_id: i64, first_question: Option<&str>) -> AppResult<()> {
    conn.execute("UPDATE conversations SET updated_at = datetime('now', 'localtime') WHERE id = ?1", [conversation_id])?;
    if let Some(question) = first_question {
        let title: String = question.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(30).collect();
        conn.execute("UPDATE conversations SET title = ?1 WHERE id = ?2 AND title = ''", params![title, conversation_id])?;
    }
    conn.execute(
        "UPDATE courses SET updated_at = datetime('now', 'localtime') WHERE id = (SELECT course_id FROM conversations WHERE id = ?1)",
        [conversation_id],
    )?;
    Ok(())
}

/// Messages left "streaming" by a crash or forced exit are marked as interrupted.
pub fn recover_interrupted(conn: &Connection) -> AppResult<usize> {
    Ok(conn.execute(
        "UPDATE messages SET status = 'error', error = '生成过程被中断（应用已关闭），可点击“重试”重新生成' WHERE status = 'streaming'",
        [],
    )?)
}

/// The last user message, used by "regenerate".
pub fn last_user_message(conn: &Connection, conversation_id: i64) -> AppResult<Option<Message>> {
    Ok(conn
        .query_row(
            &format!("SELECT {MESSAGE_COLUMNS} FROM messages WHERE conversation_id = ?1 AND role = 'user' ORDER BY id DESC LIMIT 1"),
            [conversation_id],
            message_from_row,
        )
        .optional()?)
}

pub fn delete_after(conn: &Connection, conversation_id: i64, message_id: i64) -> AppResult<()> {
    conn.execute("DELETE FROM messages WHERE conversation_id = ?1 AND id > ?2", params![conversation_id, message_id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support;

    #[test]
    fn conversations_and_messages_round_trip() {
        let (paths, conn) = test_support::course_with_document();
        let conversation = create(&conn, CreateConversation { course_id: 1, mode: None, title: None, context_mode: None, scope: None }, ContextMode::Rag).unwrap();
        assert_eq!((conversation.mode.as_str(), conversation.context_mode), ("qa", ContextMode::Rag));
        let question = insert_message(&conn, conversation.id, "user", "什么是导数？", "done", &[], &Value::Null).unwrap();
        let answer = insert_message(&conn, conversation.id, "assistant", "", "streaming", &[], &serde_json::json!({"model": "m"})).unwrap();
        save_partial(&conn, answer.id, "导数是").unwrap();
        assert_eq!(recover_interrupted(&conn).unwrap(), 1);
        let restored = get_message(&conn, answer.id).unwrap();
        assert_eq!((restored.status.as_str(), restored.content.as_str()), ("error", "导数是"));
        touch(&conn, conversation.id, Some(&question.content)).unwrap();
        let listed = list(&conn, 1).unwrap();
        assert_eq!(listed[0].title, "什么是导数？");
        assert_eq!(listed[0].message_count, 2);
        let updated = update(&conn, UpdateConversation { id: conversation.id, title: Some("导数".into()), context_mode: Some(ContextMode::Full), scope: Some(ConversationScope { document_ids: vec![1], section_ids: vec![] }), profile_id: Some(0) }).unwrap();
        assert_eq!((updated.title.as_str(), updated.context_mode, updated.scope.document_ids.clone()), ("导数", ContextMode::Full, vec![1]));
        assert!(create(&conn, CreateConversation { course_id: 1, mode: Some("hack".into()), title: None, context_mode: None, scope: None }, ContextMode::Auto).is_err());
        delete(&conn, conversation.id).unwrap();
        assert!(messages(&conn, conversation.id).unwrap().is_empty());
        drop(conn);
        test_support::cleanup(paths);
    }
}
