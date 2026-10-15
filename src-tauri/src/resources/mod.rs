//! External course videos and links. ExamOps stores the link, timestamps and the
//! student's subtitles / notes and explicitly labelled Bilibili text; it never
//! embeds the website or guesses a video's content from its URL.

use crate::context::{excerpt, BuiltContext, SourceRef};
use crate::error::{AppError, AppResult, OptionalRow};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct Mark {
    pub id: i64,
    pub resource_id: i64,
    pub seconds: i64,
    pub label: String,
    pub note: String,
    pub knowledge_point_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Resource {
    pub id: i64,
    pub course_id: i64,
    pub kind: String,
    pub title: String,
    pub url: String,
    pub section_id: Option<i64>,
    pub section_title: Option<String>,
    pub transcript: String,
    pub notes: String,
    pub marks: Vec<Mark>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResourceInput {
    pub id: Option<i64>,
    pub course_id: i64,
    #[serde(default)]
    pub kind: Option<String>,
    pub title: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub section_id: Option<i64>,
    #[serde(default)]
    pub transcript: String,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarkInput {
    pub id: Option<i64>,
    pub resource_id: i64,
    pub seconds: i64,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub knowledge_point_id: Option<i64>,
}

const SELECT: &str = "SELECT r.id, r.course_id, r.kind, r.title, r.url, r.section_id, s.title, r.transcript, r.notes, r.created_at, r.updated_at
    FROM resources r LEFT JOIN document_sections s ON s.id = r.section_id";

fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Resource> {
    Ok(Resource {
        id: row.get(0)?,
        course_id: row.get(1)?,
        kind: row.get(2)?,
        title: row.get(3)?,
        url: row.get(4)?,
        section_id: row.get(5)?,
        section_title: row.get(6)?,
        transcript: row.get(7)?,
        notes: row.get(8)?,
        marks: Vec::new(),
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

fn marks(conn: &Connection, resource_id: i64) -> AppResult<Vec<Mark>> {
    let mut stmt = conn.prepare(
        "SELECT id, resource_id, seconds, label, note, knowledge_point_id FROM resource_marks WHERE resource_id = ?1 ORDER BY seconds, id",
    )?;
    let rows = stmt.query_map([resource_id], |r| {
        Ok(Mark { id: r.get(0)?, resource_id: r.get(1)?, seconds: r.get(2)?, label: r.get(3)?, note: r.get(4)?, knowledge_point_id: r.get(5)? })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn list(conn: &Connection, course_id: i64) -> AppResult<Vec<Resource>> {
    let mut stmt = conn.prepare(&format!("{SELECT} WHERE r.course_id = ?1 ORDER BY r.id"))?;
    let rows = stmt.query_map([course_id], from_row)?;
    let mut list: Vec<Resource> = rows.collect::<rusqlite::Result<_>>()?;
    for resource in &mut list {
        resource.marks = marks(conn, resource.id)?;
    }
    Ok(list)
}

pub fn get(conn: &Connection, id: i64) -> AppResult<Resource> {
    let mut resource = conn.query_row(&format!("{SELECT} WHERE r.id = ?1"), [id], from_row).or_not_found("网课资源")?;
    resource.marks = marks(conn, id)?;
    Ok(resource)
}

fn validate_url(url: &str) -> AppResult<String> {
    let url = url.trim();
    if url.is_empty() {
        return Ok(String::new());
    }
    let parsed = url::Url::parse(url).map_err(|_| AppError::invalid("链接格式不正确，请填写以 http:// 或 https:// 开头的网址"))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(AppError::invalid("只支持 http/https 链接"));
    }
    Ok(parsed.to_string())
}

pub fn save(conn: &Connection, input: ResourceInput) -> AppResult<Resource> {
    let title = input.title.trim();
    if title.is_empty() {
        return Err(AppError::invalid("请填写网课名称"));
    }
    let url = validate_url(&input.url)?;
    let kind = match input.kind.as_deref() {
        Some("link") => "link",
        _ => "video",
    };
    if let Some(section) = input.section_id {
        let course: i64 = conn
            .query_row("SELECT d.course_id FROM document_sections s JOIN documents d ON d.id = s.document_id WHERE s.id = ?1", [section], |r| r.get(0))
            .or_not_found("章节")?;
        if course != input.course_id {
            return Err(AppError::invalid("章节不属于该课程"));
        }
    }
    let transcript: String = input.transcript.chars().take(400_000).collect();
    let notes: String = input.notes.chars().take(100_000).collect();
    match input.id {
        Some(id) => {
            conn.execute(
                "UPDATE resources SET kind = ?1, title = ?2, url = ?3, section_id = ?4, transcript = ?5, notes = ?6,
                 updated_at = datetime('now', 'localtime') WHERE id = ?7 AND course_id = ?8",
                params![kind, title, url, input.section_id, transcript, notes, id, input.course_id],
            )?;
            get(conn, id)
        }
        None => {
            conn.execute(
                "INSERT INTO resources (course_id, kind, title, url, section_id, transcript, notes) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![input.course_id, kind, title, url, input.section_id, transcript, notes],
            )?;
            get(conn, conn.last_insert_rowid())
        }
    }
}

pub fn delete(conn: &Connection, id: i64) -> AppResult<()> {
    if conn.execute("DELETE FROM resources WHERE id = ?1", [id])? == 0 {
        return Err(AppError::NotFound("网课资源"));
    }
    Ok(())
}

pub fn save_mark(conn: &Connection, input: MarkInput) -> AppResult<Resource> {
    get(conn, input.resource_id)?;
    if !(0..=24 * 3600).contains(&input.seconds) {
        return Err(AppError::invalid("时间戳超出范围"));
    }
    match input.id {
        Some(id) => conn.execute(
            "UPDATE resource_marks SET seconds = ?1, label = ?2, note = ?3, knowledge_point_id = ?4 WHERE id = ?5 AND resource_id = ?6",
            params![input.seconds, input.label.trim(), input.note.trim(), input.knowledge_point_id, id, input.resource_id],
        )?,
        None => conn.execute(
            "INSERT INTO resource_marks (resource_id, seconds, label, note, knowledge_point_id) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![input.resource_id, input.seconds, input.label.trim(), input.note.trim(), input.knowledge_point_id],
        )?,
    };
    get(conn, input.resource_id)
}

pub fn delete_mark(conn: &Connection, id: i64) -> AppResult<()> {
    conn.execute("DELETE FROM resource_marks WHERE id = ?1", [id])?;
    Ok(())
}

pub fn format_time(seconds: i64) -> String {
    let (h, m, s) = (seconds / 3600, seconds % 3600 / 60, seconds % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}

/// Characters of transcript per lesson part.
pub const PART_CHARS: usize = 1500;
const NOTES_CHARS: usize = 8000;

/// Splits the transcript into parts of about `PART_CHARS` on line boundaries.
pub fn transcript_parts(resource: &Resource) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut size = 0;
    for line in resource.transcript.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let chars: Vec<char> = line.chars().collect();
        for piece in chars.chunks(PART_CHARS) {
            if size > 0 && size + piece.len() > PART_CHARS {
                parts.push(std::mem::take(&mut current));
                size = 0;
            }
            if size > 0 {
                current.push('\n');
            }
            current.extend(piece.iter());
            size += piece.len();
        }
    }
    if size > 0 {
        parts.push(current);
    }
    parts
}

/// Indices of the transcript parts that share the most terms with `query`.
pub fn relevant_parts(parts: &[String], query: &str, limit: usize) -> Vec<usize> {
    use crate::search::segment::{highlight_terms, normalize_char, QueryMode};
    let terms = highlight_terms(query, QueryMode::Any);
    let mut scored: Vec<(usize, usize)> = parts
        .iter()
        .enumerate()
        .map(|(index, part)| {
            let normalized: String = part.chars().map(normalize_char).collect();
            (index, terms.iter().filter(|t| normalized.contains(t.as_str())).count())
        })
        .filter(|(_, score)| *score > 0)
        .collect();
    scored.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let mut picked: Vec<usize> = scored.into_iter().take(limit).map(|(i, _)| i).collect();
    picked.sort_unstable();
    picked
}

pub fn transcript_label(resource: &Resource) -> &'static str {
    let text = resource.transcript.trim_start();
    if text.starts_with(crate::bilibili::SUMMARY_HEADER) {
        "B站 AI 总结（非完整字幕）"
    } else if text.contains(crate::bilibili::SUMMARY_HEADER) {
        "文字稿（含 B站 AI 总结）"
    } else if text.starts_with(crate::bilibili::SUBTITLE_HEADER) {
        "B站字幕"
    } else {
        "字幕/文字稿"
    }
}

/// The saved transcript parts / notes / timestamps as fenced teaching
/// material. Returns `None` when there is nothing the AI may use.
pub fn material(resource: &Resource, transcript: &[usize], first_label: usize) -> Option<BuiltContext> {
    let mut parts: Vec<(String, String)> = Vec::new();
    let pieces = transcript_parts(resource);
    for index in transcript.iter().copied().filter(|i| *i < pieces.len()) {
        parts.push((format!("{} 第 {}/{} 段", transcript_label(resource), index + 1, pieces.len()), pieces[index].clone()));
    }
    if !resource.notes.trim().is_empty() {
        parts.push(("学习笔记".into(), resource.notes.trim().chars().take(NOTES_CHARS).collect()));
    }
    if !resource.marks.is_empty() {
        let lines: Vec<String> = resource
            .marks
            .iter()
            .map(|m| format!("{} {} {}", format_time(m.seconds), m.label, m.note).trim().to_string())
            .collect();
        parts.push(("时间戳笔记".into(), lines.join("\n")));
    }
    if parts.is_empty() {
        return None;
    }
    let mut text = String::new();
    let mut sources = Vec::new();
    for (offset, (kind, body)) in parts.into_iter().enumerate() {
        let label = format!("S{}", first_label + offset);
        let source = if kind == "学习笔记" || kind == "时间戳笔记" || transcript_label(resource) == "字幕/文字稿" {
            "用户提供的文字"
        } else { transcript_label(resource) };
        text.push_str(&format!(
            "<<<资料 {label} | 网课《{}》· {kind}>>>\n{}\n<<<{label} 结束>>>\n\n",
            resource.title.replace("<<<", "«<").replace(">>>", ">»"),
            body.replace("<<<", "«<").replace(">>>", ">»")
        ));
        sources.push(SourceRef {
            label,
            chunk_id: -resource.id,
            document_id: 0,
            filename: format!("网课：{}", resource.title),
            section_id: resource.section_id,
            section_title: Some(kind),
            page_start: 0,
            page_end: 0,
            loc_label: source.into(),
            excerpt: excerpt(&body, 160),
            cited: false,
        });
    }
    Some(BuiltContext {
        mode_used: "lesson".into(),
        tokens: crate::ai::tokens::estimate(&text),
        text,
        sources,
        material_tokens: 0,
        truncated: false,
        notice: None,
        pinned: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support;

    #[test]
    fn resources_marks_and_material() {
        let (paths, conn) = test_support::course_with_document();
        let chapter: i64 = conn.query_row("SELECT id FROM document_sections WHERE title = '第二章 导数'", [], |r| r.get(0)).unwrap();
        assert!(save(&conn, ResourceInput { id: None, course_id: 1, kind: None, title: "网课".into(), url: "javascript:alert(1)".into(), section_id: None, transcript: String::new(), notes: String::new() }).is_err());
        let video = save(&conn, ResourceInput {
            id: None, course_id: 1, kind: Some("video".into()), title: "导数精讲".into(), url: "https://www.bilibili.com/video/BV1xx".into(),
            section_id: Some(chapter), transcript: String::new(), notes: String::new(),
        }).unwrap();
        assert_eq!(video.section_title.as_deref(), Some("第二章 导数"));
        // No subtitles or notes: the AI gets nothing to work with.
        assert!(material(&video, &[0], 1).is_none());
        let video = save_mark(&conn, MarkInput { id: None, resource_id: video.id, seconds: 754, label: "几何意义".into(), note: "切线斜率".into(), knowledge_point_id: None }).unwrap();
        assert_eq!(video.marks.len(), 1);
        let built = material(&video, &[], 3).unwrap();
        assert!(built.text.contains("12:34 几何意义 切线斜率"));
        assert_eq!(built.sources[0].label, "S3");
        // Provenance must survive splitting, even when a later part has no header.
        let mut summary_video = video.clone();
        summary_video.transcript = format!("{}\n{}", crate::bilibili::SUMMARY_HEADER, "摘要内容".repeat(800));
        let summary = material(&summary_video, &[1], 1).unwrap();
        assert!(summary.text.contains("B站 AI 总结（非完整字幕） 第 2/"));
        assert_eq!(summary.sources[0].loc_label, "B站 AI 总结（非完整字幕）");
        // Transcripts are split on line boundaries and searched by shared terms.
        let video = save(&conn, ResourceInput {
            id: Some(video.id), course_id: 1, kind: None, title: "导数精讲".into(), url: String::new(), section_id: None,
            transcript: format!("{}\n切线斜率与导数的几何意义\n{}", "开场介绍".repeat(300), "例题讲解".repeat(300)), notes: String::new(),
        }).unwrap();
        let parts = transcript_parts(&video);
        assert_eq!(parts.len(), 2);
        assert!(parts.iter().all(|p| p.chars().count() <= PART_CHARS));
        assert_eq!(relevant_parts(&parts, "导数的几何意义是什么", 1), vec![0]);
        let built = material(&video, &[1], 1).unwrap();
        assert!(built.text.contains("字幕/文字稿 第 2/2 段") && !built.text.contains("开场介绍"));
        assert_eq!(format_time(3725), "1:02:05");
        assert!(save_mark(&conn, MarkInput { id: None, resource_id: video.id, seconds: -1, label: String::new(), note: String::new(), knowledge_point_id: None }).is_err());
        delete(&conn, video.id).unwrap();
        assert!(list(&conn, 1).unwrap().is_empty());
        drop(conn);
        test_support::cleanup(paths);
    }
}
