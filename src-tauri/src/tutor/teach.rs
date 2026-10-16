//! Teaching plans for the three study modes (Roadmap · Tutor Engine):
//!
//! * 从零学习 (learn) walks through a chapter — or the student's transcript of a
//!   video — part by part ("lesson parts"), waiting for the student after each;
//! * 正常复习 (review) uses active recall over the chapter;
//! * 考前冲刺 (sprint) condenses key points around weak spots.
//!
//! Students can ask follow-ups, ask for an example or another explanation,
//! skip or jump to a part, and switch chapters (another conversation) at any
//! time. Progress lives on the conversation and survives restarts.

use super::chat::{AfterHook, TurnPlan};
use super::{prompts, Conversation, TeachProgress};
use crate::context::{self, BuiltContext, ConversationScope};
use crate::error::{AppError, AppResult};
use crate::resources::{self, Resource};
use crate::search::ChunkHit;
use crate::{knowledge, paths, study};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::ops::Range;

pub const ACTIONS: [&str; 8] = ["start", "continue", "skip", "goto", "example", "rephrase", "summary", "ask"];
pub const TEACHING_MODES: [&str; 3] = ["learn", "review", "sprint"];
/// Characters of chapter text per lesson part.
const PART_CHARS: usize = 1500;
/// Transcript parts given to the model outside step-by-step teaching.
const VIDEO_CONTEXT_PARTS: usize = 6;

#[derive(Debug, Clone, Deserialize)]
pub struct StartTeaching {
    pub course_id: i64,
    /// learn | review | sprint
    pub mode: String,
    #[serde(default)]
    pub section_id: Option<i64>,
    #[serde(default)]
    pub resource_id: Option<i64>,
    #[serde(default)]
    pub step_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LessonPart {
    pub index: usize,
    pub title: String,
    pub loc_label: String,
    pub chars: usize,
    /// taught | skipped | next | todo
    pub status: String,
}

/// The material a teaching conversation walks through.
enum Lesson {
    Chapter { chunks: Vec<ChunkHit>, parts: Vec<Range<usize>> },
    Video { resource: Box<Resource>, parts: Vec<String> },
}

/// Groups consecutive chunks into parts of about `PART_CHARS` (at least one chunk each).
fn lesson_ranges(chunks: &[ChunkHit]) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut start = 0;
    while start < chunks.len() {
        let mut end = start;
        let mut chars = 0;
        while end < chunks.len() {
            let next = chunks[end].content.chars().count();
            if end > start && chars + next > PART_CHARS {
                break;
            }
            chars += next;
            end += 1;
        }
        ranges.push(start..end);
        start = end;
    }
    ranges
}

fn teaching_scope(conversation: &Conversation) -> ConversationScope {
    match conversation.section_id {
        Some(section) => ConversationScope { document_ids: vec![], section_ids: vec![section] },
        None => conversation.scope.clone(),
    }
}

impl Lesson {
    fn load(conn: &Connection, conversation: &Conversation) -> AppResult<Lesson> {
        if let Some(id) = conversation.resource_id {
            let resource = resources::get(conn, id)?;
            let parts = resources::transcript_parts(&resource);
            return Ok(Lesson::Video { resource: Box::new(resource), parts });
        }
        let chunks = context::scope_chunks(conn, conversation.course_id, &teaching_scope(conversation))?;
        let parts = lesson_ranges(&chunks);
        Ok(Lesson::Chapter { chunks, parts })
    }

    fn len(&self) -> usize {
        match self {
            Lesson::Chapter { parts, .. } => parts.len(),
            // Without subtitles, notes and timestamps form a single part.
            Lesson::Video { resource, parts } if parts.is_empty() => usize::from(has_notes(resource)),
            Lesson::Video { parts, .. } => parts.len(),
        }
    }

    fn video(&self) -> Option<&Resource> {
        match self {
            Lesson::Video { resource, .. } => Some(resource),
            Lesson::Chapter { .. } => None,
        }
    }

    /// Fenced material of one part, labelled from S1.
    fn part_material(&self, index: usize) -> Option<BuiltContext> {
        match self {
            Lesson::Chapter { chunks, parts } => parts.get(index).map(|range| lesson_context(&chunks[range.clone()])),
            Lesson::Video { resource, parts } => {
                let selected: Vec<usize> = if parts.is_empty() { vec![] } else { vec![index] };
                resources::material(resource, &selected, 1)
            }
        }
    }

    fn describe(&self, index: usize) -> LessonPart {
        let (title, loc_label, chars) = match self {
            Lesson::Chapter { chunks, parts } => {
                let range = parts[index].clone();
                let first = &chunks[range.start];
                let last = &chunks[range.end - 1];
                let loc = merge_labels(&first.loc_label, &last.loc_label);
                let title = first.section_title.clone().unwrap_or_else(|| first.filename.clone());
                (title, loc, chunks[range].iter().map(|c| c.content.chars().count()).sum())
            }
            Lesson::Video { resource, parts } if parts.is_empty() => ("学习笔记与时间戳".to_string(), "用户提供的文字".to_string(), resource.notes.chars().count()),
            Lesson::Video { resource, parts } => {
                let first_line: String = parts[index].lines().next().unwrap_or("").chars().take(24).collect();
                let label = match resources::transcript_label(resource) { "字幕/文字稿" => "字幕", label => label };
                (format!("{label}第 {} 段：{first_line}", index + 1), format!("第 {}/{} 段", index + 1, parts.len()), parts[index].chars().count())
            }
        };
        LessonPart { index, title, loc_label, chars, status: String::new() }
    }

    /// Sections whose last chunk lies in this part (their content is now fully taught).
    fn sections_completed_by(&self, index: usize) -> Vec<i64> {
        let Lesson::Chapter { chunks, parts } = self else { return Vec::new() };
        let Some(range) = parts.get(index) else { return Vec::new() };
        let mut done = Vec::new();
        for position in range.clone() {
            let Some(section) = chunks[position].section_id else { continue };
            let later = chunks[position + 1..].iter().any(|c| c.section_id == Some(section));
            if !later && !done.contains(&section) {
                done.push(section);
            }
        }
        done
    }
}

/// "第 1–2 行" + "第 11–20 行" → "第 1–20 行"; different units are joined with "至".
fn merge_labels(first: &str, last: &str) -> String {
    if first == last {
        return first.to_string();
    }
    let pattern = regex::Regex::new(r"^第 (\d+)(?:–(\d+))? (.+)$").expect("valid regex");
    match (pattern.captures(first), pattern.captures(last)) {
        (Some(a), Some(b)) if a[3] == b[3] => {
            let end = b.get(2).unwrap_or_else(|| b.get(1).expect("group 1")).as_str();
            format!("第 {}–{end} {}", &a[1], &a[3])
        }
        _ => format!("{first} 至 {last}"),
    }
}

fn has_notes(resource: &Resource) -> bool {
    !resource.notes.trim().is_empty() || !resource.marks.is_empty()
}

/// Labelled material of the part being taught.
fn lesson_context(chunks: &[ChunkHit]) -> BuiltContext {
    let (text, sources) = context::render(chunks, 1);
    BuiltContext {
        mode_used: "lesson".into(),
        tokens: chunks.iter().map(|c| crate::ai::tokens::estimate(&c.content) + 24).sum(),
        text: format!("【本次讲解片段】\n{text}"),
        sources,
        ..Default::default()
    }
}

/// Video material outside step-by-step teaching: the last taught part plus the
/// parts most related to the question (or the opening parts).
fn video_context(resource: &Resource, parts: &[String], last: Option<usize>, query: &str) -> Option<BuiltContext> {
    let mut selected = if query.trim().is_empty() { Vec::new() } else { resources::relevant_parts(parts, query, VIDEO_CONTEXT_PARTS - 1) };
    if let Some(last) = last.filter(|l| *l < parts.len()) {
        if !selected.contains(&last) {
            selected.push(last);
        }
    }
    if selected.is_empty() {
        selected = (0..parts.len().min(VIDEO_CONTEXT_PARTS)).collect();
    }
    selected.sort_unstable();
    resources::material(resource, &selected, 1)
}

/// Re-opens the teaching conversation for this chapter/video and mode, or creates it.
pub fn start(conn: &Connection, input: StartTeaching, default_mode: context::ContextMode) -> AppResult<Conversation> {
    if !TEACHING_MODES.contains(&input.mode.as_str()) {
        return Err(AppError::invalid("未知的学习模式"));
    }
    if input.section_id.is_none() && input.resource_id.is_none() && input.mode == "learn" {
        return Err(AppError::invalid("从零学习需要先选择章节或网课"));
    }
    let exists: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM courses WHERE id = ?1)", [input.course_id], |r| r.get(0))?;
    if !exists {
        return Err(AppError::NotFound("课程"));
    }
    let mut section_id = input.section_id;
    let mut subject = String::new();
    if let Some(resource) = input.resource_id {
        let video = resources::get(conn, resource)?;
        if video.course_id != input.course_id {
            return Err(AppError::invalid("网课不属于该课程"));
        }
        section_id = section_id.or(video.section_id);
        subject = format!("网课《{}》", video.title);
    }
    if let Some(section) = section_id {
        let (title, course): (String, i64) = conn
            .query_row(
                "SELECT s.title, d.course_id FROM document_sections s JOIN documents d ON d.id = s.document_id WHERE s.id = ?1",
                [section],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?
            .ok_or(AppError::NotFound("章节"))?;
        if course != input.course_id {
            return Err(AppError::invalid("章节不属于该课程"));
        }
        if subject.is_empty() {
            subject = title;
        }
    }
    if subject.is_empty() {
        subject = "全部内容".into();
    }
    if let Some(step) = input.step_id {
        let step = paths::get_step(conn, step)?;
        let course: i64 = conn.query_row("SELECT course_id FROM learning_paths WHERE id = ?1", [step.path_id], |r| r.get(0))?;
        if course != input.course_id {
            return Err(AppError::invalid("学习步骤不属于该课程"));
        }
    }
    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM conversations WHERE course_id = ?1 AND mode = ?2 AND section_id IS ?3 AND resource_id IS ?4 ORDER BY id DESC LIMIT 1",
            params![input.course_id, input.mode, section_id, input.resource_id],
            |r| r.get(0),
        )
        .optional()?;
    let id = match existing {
        Some(id) => {
            if input.step_id.is_some() {
                conn.execute("UPDATE conversations SET step_id = ?1 WHERE id = ?2", params![input.step_id, id])?;
            }
            id
        }
        None => {
            let label = match input.mode.as_str() {
                "review" => "复习",
                "sprint" => "冲刺",
                _ => "学习",
            };
            let scope = ConversationScope { document_ids: vec![], section_ids: section_id.into_iter().collect() };
            let context_mode = match default_mode {
                context::ContextMode::Full => "full",
                context::ContextMode::Rag => "rag",
                context::ContextMode::Auto => "auto",
            };
            conn.execute(
                "INSERT INTO conversations (course_id, mode, title, context_mode, scope, section_id, step_id, resource_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    input.course_id,
                    input.mode,
                    format!("{label}｜{subject}"),
                    context_mode,
                    serde_json::to_string(&scope)?,
                    section_id,
                    input.step_id,
                    input.resource_id
                ],
            )?;
            conn.last_insert_rowid()
        }
    };
    let conversation = super::get(conn, id)?;
    if conversation.mode == "learn" {
        // Record the number of parts so progress can be shown before the first turn.
        let parts = Lesson::load(conn, &conversation)?.len();
        if parts != conversation.progress.parts {
            super::save_progress(conn, id, &TeachProgress { parts, ..conversation.progress.clone() })?;
        }
    }
    study::set_conversation(conn, input.course_id, id, section_id)?;
    if let Some(step) = input.step_id {
        begin_step(conn, step)?;
    }
    super::get(conn, id)
}

/// Lesson parts with their status, for the chapter map beside the chat.
pub fn outline(conn: &Connection, conversation_id: i64) -> AppResult<Vec<LessonPart>> {
    let conversation = super::get(conn, conversation_id)?;
    if conversation.mode != "learn" {
        return Ok(Vec::new());
    }
    let lesson = Lesson::load(conn, &conversation)?;
    let progress = &conversation.progress;
    Ok((0..lesson.len())
        .map(|index| {
            let mut part = lesson.describe(index);
            part.status = if progress.taught.contains(&index) {
                "taught"
            } else if progress.skipped.contains(&index) {
                "skipped"
            } else if index == progress.next && !progress.finished {
                "next"
            } else {
                "todo"
            }
            .into();
            part
        })
        .collect())
}

fn chapter_outline(conn: &Connection, section_id: i64) -> AppResult<String> {
    let mut stmt = conn.prepare(
        "WITH RECURSIVE tree(id, depth) AS (SELECT ?1, 0 UNION ALL SELECT s.id, tree.depth + 1 FROM document_sections s JOIN tree ON s.parent_id = tree.id)
         SELECT s.title, tree.depth FROM tree JOIN document_sections s ON s.id = tree.id ORDER BY s.ordinal",
    )?;
    let rows = stmt.query_map([section_id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
    let mut out = String::new();
    for row in rows.take(40) {
        let (title, depth) = row?;
        out.push_str(&format!("{}- {title}\n", "  ".repeat(depth as usize)));
    }
    Ok(out)
}

fn status_name(status: &str) -> &'static str {
    match status {
        "mastered" => "已掌握",
        "learned" => "学过",
        _ => "未学",
    }
}

/// What the tutor knows about the student's knowledge points in this scope.
fn student_state(points: &[knowledge::KnowledgePoint]) -> String {
    if points.is_empty() {
        return String::new();
    }
    let mut out = String::from("【学生的知识点状态】\n");
    for point in points.iter().take(40) {
        out.push_str(&format!(
            "- {}（{}{}）\n",
            point.title,
            status_name(&point.status),
            if point.difficult { "，学生标记为疑难" } else { "" }
        ));
    }
    out
}

fn exam_line(conn: &Connection, course_id: i64) -> AppResult<String> {
    let date: Option<String> = conn.query_row("SELECT exam_date FROM courses WHERE id = ?1", [course_id], |r| r.get(0))?;
    Ok(match date.and_then(|d| chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()) {
        Some(day) => match (day - chrono::Local::now().date_naive()).num_days() {
            left if left > 0 => format!("距离考试还有 {left} 天。"),
            0 => "今天就要考试了。".into(),
            _ => "考试日期已过。".into(),
        },
        None => "学生没有设置考试日期。".into(),
    })
}

fn action_label(mode: &str, video: bool, action: &str) -> &'static str {
    match (mode, action) {
        (_, "example") => "举个例子",
        (_, "rephrase") => "没太懂，换种方式讲讲",
        (_, "summary") => "总结一下要点",
        ("learn", "start") if video => "开始学习这节网课",
        ("learn", "start") => "开始学习本章",
        ("learn", "continue") => "继续讲解下一部分",
        ("learn", "skip") => "下一部分我已经会了，跳过它",
        ("review", "start") => "开始复习",
        ("review", _) => "再来一组复习题",
        ("sprint", "start") => "开始考前冲刺",
        ("sprint", _) => "继续冲刺：讲一道典型题",
        _ => "继续",
    }
}

#[derive(Debug, Clone, Copy)]
struct Ids {
    conversation: i64,
    course: i64,
    section: Option<i64>,
    step: Option<i64>,
}

/// Builds the turn for a teaching conversation.
pub fn plan(conn: &Connection, conversation: &Conversation, action: &str, target: Option<usize>, text: &str) -> AppResult<TurnPlan> {
    let action = if ACTIONS.contains(&action) { action } else { "ask" };
    let question = text.trim();
    if action == "ask" && question.is_empty() {
        return Err(AppError::invalid("请输入问题"));
    }
    let lesson = Lesson::load(conn, conversation)?;
    let total = lesson.len();
    let before = conversation.progress.clone();
    let mode = conversation.mode.as_str();
    let points = match conversation.section_id {
        Some(section) => knowledge::in_section(conn, section)?,
        None if conversation.resource_id.is_none() => knowledge::list(conn, conversation.course_id)?,
        None => Vec::new(),
    };

    let mut task = String::new();
    let mut primary: Option<BuiltContext> = None;
    let mut retrieve = true;
    let mut after: Option<AfterHook> = None;
    let mut taught: Option<usize> = None;
    // Knowledge points fully taught this turn (the UI may suggest a quiz on them).
    let mut covered: Vec<i64> = Vec::new();
    let mut display = action_label(mode, lesson.video().is_some(), action).to_string();

    let ids = Ids { conversation: conversation.id, course: conversation.course_id, section: conversation.section_id, step: conversation.step_id };
    let stepwise = mode == "learn" && matches!(action, "start" | "continue" | "skip" | "goto");
    if stepwise && total == 0 {
        if lesson.video().is_none() {
            return Err(AppError::invalid("这部分没有可讲解的文字内容（可能是扫描件或图片），请换一个章节，或直接提问"));
        }
        // A video without subtitles or notes: never guess its content.
        task.push_str("请先说明你无法获知这节网课的具体内容，然后依据课程资料简要介绍网课关联章节的要点。");
    } else if stepwise {
        let mut next = before.next.min(total);
        let mut progress = TeachProgress { parts: total, ..before.clone() };
        match action {
            "skip" if next < total => {
                if !progress.skipped.contains(&next) && !progress.taught.contains(&next) {
                    progress.skipped.push(next);
                }
                next += 1;
            }
            "goto" => {
                let index = target.filter(|t| *t < total).ok_or_else(|| AppError::invalid("没有这一部分"))?;
                next = index;
                display = format!("讲解第 {} 部分：{}", index + 1, lesson.describe(index).title);
            }
            _ => {}
        }
        if next >= total {
            task.push_str(prompts::LEARN_FINISHED);
            progress.next = total;
            progress.finished = true;
            match lesson.video() {
                Some(video) => {
                    let resource = video.id;
                    let marked: Vec<i64> = video.marks.iter().filter_map(|m| m.knowledge_point_id).collect();
                    after = Some(Box::new(move |conn, _| {
                        super::save_progress(conn, ids.conversation, &progress)?;
                        knowledge::mark_learned(conn, &marked)?;
                        finish(conn, ids, Some(resource), false)
                    }));
                }
                None => {
                    after = Some(Box::new(move |conn, _| {
                        super::save_progress(conn, ids.conversation, &progress)?;
                        finish(conn, ids, None, true)
                    }));
                }
            }
        } else {
            let part = lesson.describe(next);
            let label = format!("第 {}/{total} 部分「{}」（{}）", next + 1, part.title, part.loc_label);
            task.push_str(&prompts::LEARN_SLICE.replace("{part}", &label));
            if lesson.video().is_some() {
                task.push_str(prompts::VIDEO_LESSON);
            }
            if next == 0 {
                task.push_str(prompts::LEARN_FIRST);
            }
            if next + 1 == total {
                task.push_str(prompts::LEARN_LAST);
            }
            primary = lesson.part_material(next);
            // Chapters are taught from the part alone; videos are related to the course text.
            retrieve = lesson.video().is_some();
            progress.next = next + 1;
            progress.last = Some(next);
            progress.skipped.retain(|p| *p != next);
            if !progress.taught.contains(&next) {
                progress.taught.push(next);
            }
            taught = Some(next);
            let sections = lesson.sections_completed_by(next);
            let completed: Vec<i64> = points.iter().filter(|p| p.section_id.map(|s| sections.contains(&s)).unwrap_or(false)).map(|p| p.id).collect();
            covered = completed.clone();
            after = Some(Box::new(move |conn, _| {
                super::save_progress(conn, ids.conversation, &progress)?;
                knowledge::mark_learned(conn, &completed)?;
                study::record(conn, ids.course, "explain", study::EventRefs { section_id: ids.section, step_id: ids.step, ..Default::default() }, json!({ "part": next, "parts": progress.parts }))?;
                if let Some(step) = ids.step {
                    begin_step(conn, step)?;
                }
                Ok(())
            }));
        }
    } else if mode == "learn" {
        task.push_str(match action {
            "example" => prompts::LEARN_EXAMPLE,
            "rephrase" => prompts::LEARN_REPHRASE,
            "summary" => prompts::LEARN_SUMMARY,
            _ => prompts::LEARN_ASK,
        });
        let last = before.last.filter(|l| *l < total);
        match &lesson {
            Lesson::Video { resource, parts } => primary = video_context(resource, parts, last, if action == "ask" { question } else { "" }),
            // A summary uses the whole chapter (Context Engine); the rest centre on the last part.
            Lesson::Chapter { .. } if action != "summary" => primary = last.and_then(|l| lesson.part_material(l)),
            Lesson::Chapter { .. } => {}
        }
    } else {
        task.push_str(&match (mode, action) {
            (_, "summary") => prompts::LEARN_SUMMARY.to_string(),
            (_, "example") => prompts::LEARN_EXAMPLE.replace("【本次讲解片段】中刚讲过的内容", "刚才复习的内容"),
            (_, "rephrase") => prompts::LEARN_REPHRASE.replace("【本次讲解片段】的内容", "刚才的内容"),
            ("review", "start") => prompts::REVIEW_START.to_string(),
            ("review", "ask") => prompts::REVIEW_ASK.to_string(),
            ("review", _) => prompts::REVIEW_CONTINUE.to_string(),
            (_, "start") => prompts::SPRINT_START.replace("{exam}", &exam_line(conn, conversation.course_id)?),
            (_, "ask") => prompts::SPRINT_ASK.to_string(),
            _ => prompts::SPRINT_CONTINUE.to_string(),
        });
        if let Lesson::Video { resource, parts } = &lesson {
            primary = video_context(resource, parts, None, question);
        }
        if action == "start" {
            let kind = if mode == "review" { "review_session" } else { "sprint_session" };
            after = Some(Box::new(move |conn, _| {
                study::record(conn, ids.course, kind, study::EventRefs { section_id: ids.section, step_id: ids.step, ..Default::default() }, json!({}))?;
                if let Some(step) = ids.step {
                    begin_step(conn, step)?;
                }
                Ok(())
            }));
        }
    }

    if let Some(video) = lesson.video() {
        task.push('\n');
        task.push_str(prompts::VIDEO_NOTE);
        if total == 0 {
            task.push_str(prompts::VIDEO_MISSING);
        }
        if !video.url.is_empty() {
            task.push_str(&format!("（视频链接仅供学生打开观看：{}）", video.url));
        }
    }
    if let Some(section) = conversation.section_id {
        task.push_str(&format!("\n\n【章节结构】\n{}", chapter_outline(conn, section)?));
    }
    let state = student_state(&points);
    if !state.is_empty() {
        task.push('\n');
        task.push_str(&state);
    }

    // Retrieval (RAG mode) looks for the chapter's key terms unless a question was asked.
    let retrieval_query = if action == "ask" {
        question.to_string()
    } else {
        let mut terms: Vec<String> = points.iter().filter(|p| mode != "sprint" || p.status != "mastered").take(20).map(|p| p.title.clone()).collect();
        terms.push(conversation.section_title.clone().unwrap_or_else(|| conversation.title.clone()));
        terms.join(" ")
    };
    let prompt = if action == "ask" { question.to_string() } else { format!("{display}。") };
    Ok(TurnPlan {
        display: if action == "ask" { question.to_string() } else { display },
        prompt,
        task,
        retrieval_query,
        scope: teaching_scope(conversation),
        primary,
        retrieve,
        meta: json!({ "action": action, "target": target, "part": taught, "parts": total, "before": before, "covered_points": covered }),
        after,
    })
}

/// Marks a path step as started when the student begins working on it.
fn begin_step(conn: &Connection, step_id: i64) -> AppResult<()> {
    if let Ok(step) = paths::get_step(conn, step_id) {
        if step.status == "todo" {
            paths::set_status(conn, step_id, "in_progress", None)?;
        }
    }
    Ok(())
}

/// A lesson taught to the end: its knowledge points count as studied ("学过",
/// never "已掌握") and the linked learning step is completed.
fn finish(conn: &Connection, ids: Ids, resource: Option<i64>, chapter: bool) -> AppResult<()> {
    if chapter {
        if let Some(section) = ids.section {
            let points: Vec<i64> = knowledge::in_section(conn, section)?.into_iter().map(|k| k.id).collect();
            knowledge::mark_learned(conn, &points)?;
        }
    }
    study::record(conn, ids.course, "lesson_finished", study::EventRefs { section_id: ids.section, step_id: ids.step, ..Default::default() }, json!({ "resource_id": resource }))?;
    if let Some(step_id) = ids.step {
        if let Ok(step) = paths::get_step(conn, step_id) {
            if step.status != "done" && matches!(step.kind.as_str(), "ai_explain" | "video") {
                paths::set_status(conn, step_id, "done", None)?;
            }
        }
    }
    Ok(())
}

/// "标记已学完": the student finishes the lesson without going through every part.
pub fn mark_finished(conn: &Connection, conversation_id: i64) -> AppResult<Conversation> {
    let conversation = super::get(conn, conversation_id)?;
    if conversation.mode != "learn" {
        return Err(AppError::invalid("只有“从零学习”对话可以标记学完"));
    }
    let lesson = Lesson::load(conn, &conversation)?;
    let progress = TeachProgress { next: lesson.len(), parts: lesson.len(), finished: true, ..conversation.progress.clone() };
    super::save_progress(conn, conversation_id, &progress)?;
    let ids = Ids { conversation: conversation.id, course: conversation.course_id, section: conversation.section_id, step: conversation.step_id };
    match lesson.video() {
        Some(video) => {
            let points: Vec<i64> = video.marks.iter().filter_map(|m| m.knowledge_point_id).collect();
            knowledge::mark_learned(conn, &points)?;
            finish(conn, ids, Some(video.id), false)?;
        }
        None => finish(conn, ids, None, true)?,
    }
    super::get(conn, conversation_id)
}

/// Before regenerating, restore the progress the last turn started from and
/// return its action and target.
pub fn prepare_regenerate(conn: &Connection, conversation_id: i64) -> AppResult<(String, Option<usize>)> {
    let last = super::last_user_message(conn, conversation_id)?.ok_or_else(|| AppError::invalid("还没有可以重新生成的问题"))?;
    let plan = last.meta.get("plan").cloned().unwrap_or(Value::Null);
    let action = plan.get("action").and_then(Value::as_str).unwrap_or("ask").to_string();
    let target = plan.get("target").and_then(Value::as_u64).map(|t| t as usize);
    if let Some(before) = plan.get("before").and_then(|b| serde_json::from_value::<TeachProgress>(b.clone()).ok()) {
        super::save_progress(conn, conversation_id, &before)?;
    }
    Ok((action, target))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::{profiles, AiState};
    use crate::security::MemoryStore;
    use crate::test_support::{self, MockReply, MockServer};
    use crate::tutor::chat;
    use std::sync::Arc;

    fn section(conn: &Connection, title: &str) -> i64 {
        conn.query_row("SELECT id FROM document_sections WHERE title = ?1", [title], |r| r.get(0)).unwrap()
    }

    fn learn(section_id: Option<i64>, resource_id: Option<i64>) -> StartTeaching {
        StartTeaching { course_id: 1, mode: "learn".into(), section_id, resource_id, step_id: None }
    }

    #[test]
    fn lesson_parts_group_chunks_by_size() {
        let chunk = |n: usize| ChunkHit {
            chunk_id: n as i64, document_id: 1, filename: "a".into(), section_id: Some(1), section_title: None, chunk_index: n as i64,
            page_start: 1, page_end: 1, loc_label: "第 1 页".into(), content: "字".repeat(if n == 2 { 2000 } else { 600 }), score: 0.0,
            unreliable_formula: false,
        };
        let chunks: Vec<_> = (0..5).map(chunk).collect();
        // 600+600 | 2000 (oversized alone) | 600+600
        assert_eq!(lesson_ranges(&chunks), vec![0..2, 2..3, 3..5]);
        assert!(lesson_ranges(&[]).is_empty());
    }

    #[test]
    fn part_locations_are_merged() {
        assert_eq!(merge_labels("第 1–2 行", "第 11–20 行"), "第 1–20 行");
        assert_eq!(merge_labels("第 3 页", "第 5 页"), "第 3–5 页");
        assert_eq!(merge_labels("第 2 张幻灯片", "第 2 张幻灯片"), "第 2 张幻灯片");
        assert_eq!(merge_labels("第 3 页", "第 4 段"), "第 3 页 至 第 4 段");
    }

    #[test]
    fn teaching_conversations_are_reused_per_chapter_and_mode() {
        let (paths, conn) = test_support::course_with_document();
        let chapter = section(&conn, "第二章 导数");
        let first = start(&conn, learn(Some(chapter), None), context::ContextMode::Auto).unwrap();
        assert_eq!(first.title, "学习｜第二章 导数");
        assert_eq!(first.scope.section_ids, vec![chapter]);
        assert_eq!(first.progress.parts, 1);
        assert_eq!(start(&conn, learn(Some(chapter), None), context::ContextMode::Auto).unwrap().id, first.id);
        let review = start(&conn, StartTeaching { mode: "review".into(), ..learn(Some(chapter), None) }, context::ContextMode::Auto).unwrap();
        assert_ne!(review.id, first.id);
        assert!(start(&conn, learn(None, None), context::ContextMode::Auto).is_err());
        assert!(start(&conn, StartTeaching { course_id: 2, ..learn(Some(chapter), None) }, context::ContextMode::Auto).is_err());
        assert_eq!(study::state(&conn, 1).unwrap().last_conversation.unwrap().id, review.id);
        drop(conn);
        test_support::cleanup(paths);
    }

    #[test]
    fn learn_plans_walk_parts_and_finish_the_chapter() {
        let (paths, conn) = test_support::course_with_document();
        let chapter = section(&conn, "第一章 函数与极限");
        knowledge::from_headings(&conn, 1, None).unwrap();
        let conversation = start(&conn, learn(Some(chapter), None), context::ContextMode::Auto).unwrap();
        let first = plan(&conn, &conversation, "start", None, "").unwrap();
        assert_eq!(first.display, "开始学习本章");
        assert!(!first.retrieve);
        let primary = first.primary.as_ref().unwrap();
        // Chapter one (intro + 1.1) fits one part.
        assert_eq!(primary.sources.len(), 2);
        assert!(first.task.contains("第 1/1 部分") && first.task.contains("这是第一部分") && first.task.contains("这是最后一部分"));
        assert!(first.task.contains("【章节结构】") && first.task.contains("1.1 极限的定义（未学）"));
        let message = crate::tutor::insert_message(&conn, conversation.id, "assistant", "讲解", "done", &[], &Value::Null).unwrap();
        (first.after.unwrap())(&conn, &message).unwrap();
        let conversation = crate::tutor::get(&conn, conversation.id).unwrap();
        assert_eq!((conversation.progress.next, conversation.progress.taught.clone(), conversation.progress.last), (1, vec![0], Some(0)));
        assert_eq!(first.meta["covered_points"].as_array().unwrap().len(), 1);
        // The part covered sub-section 1.1 entirely: its point is now "学过".
        assert_eq!(knowledge::list(&conn, 1).unwrap().iter().find(|p| p.title == "1.1 极限的定义").unwrap().status, "learned");
        let outline = outline(&conn, conversation.id).unwrap();
        assert_eq!((outline.len(), outline[0].status.as_str(), outline[0].loc_label.as_str()), (1, "taught", "第 1–2 页"));

        // Follow-ups centre on the last part; "continue" past the end wraps up.
        let example = plan(&conn, &conversation, "example", None, "").unwrap();
        assert!(example.task.contains("给出 1–2 个") && example.primary.is_some() && example.retrieve);
        let finished = plan(&conn, &conversation, "continue", None, "").unwrap();
        assert!(finished.task.contains("全部讲解完毕") && finished.primary.is_none());
        (finished.after.unwrap())(&conn, &message).unwrap();
        assert!(crate::tutor::get(&conn, conversation.id).unwrap().progress.finished);
        assert!(plan(&conn, &conversation, "ask", None, " ").is_err());
        assert!(plan(&conn, &conversation, "goto", Some(5), "").is_err());
        drop(conn);
        test_support::cleanup(paths);
    }

    #[test]
    fn skipping_and_regenerating_restore_the_right_part() {
        let (paths, conn) = test_support::course_with_document();
        let video = resources::save(&conn, resources::ResourceInput {
            id: None, course_id: 1, kind: None, title: "导数精讲".into(), url: "https://example.com/v".into(), section_id: None,
            transcript: format!("{}\n{}\n{}", "甲".repeat(1400), "乙".repeat(1400), "丙".repeat(1400)), notes: String::new(),
        }).unwrap();
        let conversation = start(&conn, learn(None, Some(video.id)), context::ContextMode::Auto).unwrap();
        assert_eq!(conversation.progress.parts, 3);
        let skip = plan(&conn, &conversation, "skip", None, "").unwrap();
        assert_eq!(skip.meta["part"], 1);
        assert!(skip.retrieve && skip.task.contains("绝不能根据视频标题或链接推测"));
        assert!(skip.primary.as_ref().unwrap().text.contains("字幕/文字稿 第 2/3 段"));
        let message = crate::tutor::insert_message(&conn, conversation.id, "user", "下一部分我已经会了，跳过它", "done", &[], &json!({ "plan": skip.meta.clone() })).unwrap();
        (skip.after.unwrap())(&conn, &message).unwrap();
        let after = crate::tutor::get(&conn, conversation.id).unwrap().progress;
        assert_eq!((after.next, after.skipped.clone(), after.taught.clone()), (2, vec![0], vec![1]));
        // Regenerate puts the progress back to where that turn started.
        assert_eq!(prepare_regenerate(&conn, conversation.id).unwrap(), ("skip".to_string(), None));
        assert_eq!(crate::tutor::get(&conn, conversation.id).unwrap().progress, TeachProgress { parts: 3, ..Default::default() });
        let goto = plan(&conn, &crate::tutor::get(&conn, conversation.id).unwrap(), "goto", Some(2), "").unwrap();
        assert!(goto.display.starts_with("讲解第 3 部分"));
        drop(conn);
        test_support::cleanup(paths);
    }

    #[test]
    fn videos_without_transcripts_are_never_guessed() {
        let (paths, conn) = test_support::course_with_document();
        let video = resources::save(&conn, resources::ResourceInput {
            id: None, course_id: 1, kind: None, title: "B 站导数课".into(), url: "https://www.bilibili.com/video/BV1".into(),
            section_id: Some(section(&conn, "第二章 导数")), transcript: String::new(), notes: String::new(),
        }).unwrap();
        let conversation = start(&conn, learn(None, Some(video.id)), context::ContextMode::Auto).unwrap();
        assert_eq!(conversation.section_title.as_deref(), Some("第二章 导数"));
        assert_eq!(conversation.title, "学习｜网课《B 站导数课》");
        let plan = plan(&conn, &conversation, "start", None, "").unwrap();
        assert!(plan.task.contains("绝不能根据视频标题或链接推测") && plan.task.contains("没有提供字幕"));
        assert!(plan.primary.is_none() && plan.retrieve && plan.after.is_none());
        drop(conn);
        test_support::cleanup(paths);
    }

    #[test]
    fn finishing_completes_the_linked_step() {
        let (paths, mut conn) = test_support::course_with_document();
        let chapter = section(&conn, "第二章 导数");
        knowledge::from_headings(&conn, 1, None).unwrap();
        let detail = paths::from_template(&mut conn, paths::TemplateInput { course_id: 1, name: None, mode: Some("sprint".into()), document_ids: vec![], include_quiz: false }).unwrap();
        let step = detail.steps.iter().find(|s| s.kind == "ai_explain" && s.section_id == Some(chapter)).unwrap();
        let conversation = start(&conn, StartTeaching { step_id: Some(step.id), ..learn(Some(chapter), None) }, context::ContextMode::Auto).unwrap();
        assert_eq!(paths::get_step(&conn, step.id).unwrap().status, "in_progress");
        let done = mark_finished(&conn, conversation.id).unwrap();
        assert!(done.progress.finished);
        assert_eq!(paths::get_step(&conn, step.id).unwrap().status, "done");
        assert_eq!(knowledge::list(&conn, 1).unwrap().iter().find(|p| p.title == "第二章 导数").unwrap().status, "learned");
        drop(conn);
        test_support::cleanup(paths);
    }

    #[test]
    fn review_and_sprint_use_the_context_engine_and_student_state() {
        let (paths, conn) = test_support::course_with_document();
        knowledge::from_headings(&conn, 1, None).unwrap();
        conn.execute("UPDATE courses SET exam_date = date('now', 'localtime', '+5 days') WHERE id = 1", []).unwrap();
        let sprint = start(&conn, StartTeaching { course_id: 1, mode: "sprint".into(), section_id: None, resource_id: None, step_id: None }, context::ContextMode::Auto).unwrap();
        assert_eq!(sprint.title, "冲刺｜全部内容");
        let plan = plan(&conn, &sprint, "start", None, "").unwrap();
        assert!(plan.task.contains("距离考试还有 5 天") && plan.task.contains("第二章 导数（未学）"));
        assert!(plan.retrieve && plan.primary.is_none() && plan.retrieval_query.contains("极限"));
        drop(conn);
        test_support::cleanup(paths);
    }

    #[tokio::test]
    async fn a_learning_turn_advances_only_after_a_complete_answer() {
        // The first turn fails even after the gateway's one automatic retry.
        let server = MockServer::start(vec![
            MockReply::Status(500, "boom".into()),
            MockReply::Status(500, "boom".into()),
            MockReply::Stream(vec!["【教材】导数是变化率 [S1]"]),
        ])
        .await;
        let (paths, mut conn) = test_support::course_with_document();
        let ai = AiState::new(Arc::new(MemoryStore::default()));
        profiles::save(&mut conn, ai.secrets.as_ref(), profiles::ProfileInput {
            id: None, name: "Mock".into(), base_url: server.url(), model: "m".into(), temperature: 0.2, top_p: None,
            max_output_tokens: 512, context_window: 16000, timeout_secs: 10, json_mode: false, api_key: None,
        }).unwrap();
        let chapter = section(&conn, "第二章 导数");
        let conversation = start(&conn, learn(Some(chapter), None), context::ContextMode::Auto).unwrap();
        drop(conn);
        let failed = chat::run_turn(&paths, &ai, conversation.id, |c, conv, t| plan(c, conv, "start", None, t), "", false, |_| {}).await.unwrap();
        assert_eq!(failed.status, "error");
        let conn = paths.connect().unwrap();
        assert_eq!(crate::tutor::get(&conn, conversation.id).unwrap().progress.next, 0);
        // Regenerate after the failure re-runs the same action.
        let (action, target) = prepare_regenerate(&conn, conversation.id).unwrap();
        drop(conn);
        let ok = chat::run_turn(&paths, &ai, conversation.id, |c, conv, t| plan(c, conv, &action, target, t), "", true, |_| {}).await.unwrap();
        assert_eq!(ok.status, "done");
        assert_eq!(ok.meta["context_mode"], "lesson");
        assert!(ok.sources.len() == 1 && ok.sources[0].cited && ok.sources[0].loc_label == "第 3 页");
        let conn = paths.connect().unwrap();
        let progress = crate::tutor::get(&conn, conversation.id).unwrap().progress;
        assert_eq!((progress.next, progress.parts), (1, 1));
        let body: Value = serde_json::from_str(&server.last_request().body).unwrap();
        let system = body["messages"][0]["content"].as_str().unwrap();
        assert!(system.contains("【本次讲解片段】") && system.contains("以下为本次学习使用的资料"));
        assert_eq!(body["messages"].as_array().unwrap().last().unwrap()["content"], "开始学习本章。");
        let messages = crate::tutor::messages(&conn, conversation.id).unwrap();
        assert_eq!(messages.iter().map(|m| m.content.as_str()).collect::<Vec<_>>(), vec!["开始学习本章", "【教材】导数是变化率 [S1]"]);
        drop(conn);
        test_support::cleanup(paths);
    }
}
