//! Assessment Engine: AI-generated quizzes over knowledge points, local
//! grading of objective questions, AI feedback for written answers (with
//! self-grading as a fallback), and the link to mastery, spaced review and
//! the mistake book.
//!
//! Life cycle: `create` (AI writes the questions) → `save_answer` while the
//! student works (progress survives restarts) → `submit` (objective grading +
//! AI grading) → `finalize` once every answer has a score: the quiz score,
//! knowledge mastery, review dates, mistakes and the learning step are updated
//! exactly once.

pub mod generate;
pub mod grade;

use crate::ai::{self, client::{ChatMessage, ChatOptions}, profiles, AiState};
use crate::context::{self, SourceRef};
use crate::database::AppPaths;
use crate::error::{AppError, AppResult, OptionalRow};
use crate::knowledge::{self, KnowledgeInput, KnowledgePoint};
use crate::mastery::{self, Evidence, PASS};
use crate::search::{self, segment::QueryMode, ChunkHit, Scope};
use crate::{paths, settings, study};
use generate::{DraftQuestion, TargetPoint};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio_util::sync::CancellationToken;

pub const ORIGINS: [&str; 6] = ["manual", "point", "chapter", "review", "mistakes", "step"];
const MAX_RESPONSE_CHARS: usize = 4000;

#[derive(Debug, Clone, Serialize)]
pub struct Quiz {
    pub id: i64,
    pub course_id: i64,
    pub title: String,
    pub origin: String,
    pub section_id: Option<i64>,
    pub section_title: Option<String>,
    pub step_id: Option<i64>,
    /// ready (answering) | grading | graded
    pub status: String,
    pub allow_skip: bool,
    /// Percentage once graded.
    pub score: Option<f64>,
    /// Written answers still waiting for a score.
    pub pending: i64,
    pub current_index: i64,
    pub question_count: i64,
    pub answered_count: i64,
    pub grading_error: Option<String>,
    pub model: Option<String>,
    pub created_at: String,
    pub submitted_at: Option<String>,
    pub graded_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Question {
    pub id: i64,
    pub quiz_id: i64,
    pub position: i64,
    pub kind: String,
    pub kind_label: String,
    pub knowledge_point_id: Option<i64>,
    pub knowledge_point_title: Option<String>,
    pub prompt: String,
    pub options: Vec<String>,
    pub response: Option<String>,
    pub skipped: bool,
    /// Revealed after submission.
    pub answer: Option<String>,
    pub explanation: Option<String>,
    pub rubric: Vec<String>,
    pub sources: Vec<SourceRef>,
    pub score: Option<f64>,
    /// auto | ai | self | pending | ''
    pub grading: String,
    pub feedback: String,
}

/// How a knowledge point moved because of this quiz.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointOutcome {
    pub knowledge_point_id: i64,
    pub title: String,
    pub result: f64,
    pub before: Option<f64>,
    pub after: f64,
    pub status: String,
    pub due_date: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct QuizDetail {
    pub quiz: Quiz,
    pub questions: Vec<Question>,
    pub sources: Vec<SourceRef>,
    pub outcome: Vec<PointOutcome>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct CreateQuiz {
    pub course_id: i64,
    #[serde(default)]
    pub origin: Option<String>,
    #[serde(default)]
    pub section_id: Option<i64>,
    #[serde(default)]
    pub knowledge_point_ids: Vec<i64>,
    #[serde(default)]
    pub step_id: Option<i64>,
    #[serde(default)]
    pub question_count: Option<usize>,
    #[serde(default)]
    pub kinds: Option<Vec<String>>,
    #[serde(default)]
    pub allow_skip: Option<bool>,
}

const QUIZ_SELECT: &str = "SELECT q.id, q.course_id, q.title, q.origin, q.section_id, s.title, q.step_id, q.status, q.allow_skip, q.score,
    q.pending, q.current_index,
    (SELECT COUNT(*) FROM quiz_questions x WHERE x.quiz_id = q.id),
    (SELECT COUNT(*) FROM quiz_questions x WHERE x.quiz_id = q.id AND (x.response IS NOT NULL OR x.skipped = 1)),
    q.grading_error, q.model, q.created_at, q.submitted_at, q.graded_at
    FROM quizzes q LEFT JOIN document_sections s ON s.id = q.section_id";

fn quiz_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Quiz> {
    Ok(Quiz {
        id: row.get(0)?,
        course_id: row.get(1)?,
        title: row.get(2)?,
        origin: row.get(3)?,
        section_id: row.get(4)?,
        section_title: row.get(5)?,
        step_id: row.get(6)?,
        status: row.get(7)?,
        allow_skip: row.get(8)?,
        score: row.get(9)?,
        pending: row.get(10)?,
        current_index: row.get(11)?,
        question_count: row.get(12)?,
        answered_count: row.get(13)?,
        grading_error: row.get(14)?,
        model: row.get(15)?,
        created_at: row.get(16)?,
        submitted_at: row.get(17)?,
        graded_at: row.get(18)?,
    })
}

const QUESTION_SELECT: &str = "SELECT x.id, x.quiz_id, x.position, x.kind, x.knowledge_point_id, k.title, x.prompt, x.options, x.response, x.skipped,
    x.answer, x.explanation, x.rubric, x.sources, x.score, x.grading, x.feedback
    FROM quiz_questions x LEFT JOIN knowledge_points k ON k.id = x.knowledge_point_id";

fn question_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Question> {
    let kind: String = row.get(3)?;
    Ok(Question {
        id: row.get(0)?,
        quiz_id: row.get(1)?,
        position: row.get(2)?,
        kind_label: generate::kind_label(&kind).to_string(),
        kind,
        knowledge_point_id: row.get(4)?,
        knowledge_point_title: row.get(5)?,
        prompt: row.get(6)?,
        options: serde_json::from_str(&row.get::<_, String>(7)?).unwrap_or_default(),
        response: row.get(8)?,
        skipped: row.get(9)?,
        answer: Some(row.get(10)?),
        explanation: Some(row.get(11)?),
        rubric: serde_json::from_str(&row.get::<_, String>(12)?).unwrap_or_default(),
        sources: serde_json::from_str(&row.get::<_, String>(13)?).unwrap_or_default(),
        score: row.get(14)?,
        grading: row.get(15)?,
        feedback: row.get(16)?,
    })
}

pub fn list(conn: &Connection, course_id: i64) -> AppResult<Vec<Quiz>> {
    let mut stmt = conn.prepare(&format!("{QUIZ_SELECT} WHERE q.course_id = ?1 ORDER BY q.created_at DESC, q.id DESC"))?;
    let rows = stmt.query_map([course_id], quiz_from_row)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn get(conn: &Connection, id: i64) -> AppResult<Quiz> {
    conn.query_row(&format!("{QUIZ_SELECT} WHERE q.id = ?1"), [id], quiz_from_row).or_not_found("小测")
}

fn questions(conn: &Connection, quiz_id: i64) -> AppResult<Vec<Question>> {
    let mut stmt = conn.prepare(&format!("{QUESTION_SELECT} WHERE x.quiz_id = ?1 ORDER BY x.position"))?;
    let rows = stmt.query_map([quiz_id], question_from_row)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

fn get_question(conn: &Connection, id: i64) -> AppResult<Question> {
    conn.query_row(&format!("{QUESTION_SELECT} WHERE x.id = ?1"), [id], question_from_row).or_not_found("题目")
}

/// The quiz as the student may see it: answers stay hidden until submission.
pub fn detail(conn: &Connection, id: i64) -> AppResult<QuizDetail> {
    let quiz = get(conn, id)?;
    let (sources, outcome): (String, String) = conn.query_row("SELECT sources, outcome FROM quizzes WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?)))?;
    let mut questions = questions(conn, id)?;
    if quiz.status == "ready" {
        for question in &mut questions {
            question.answer = None;
            question.explanation = None;
            question.rubric.clear();
            question.sources.clear();
        }
    }
    Ok(QuizDetail {
        sources: if quiz.status == "ready" { Vec::new() } else { serde_json::from_str(&sources).unwrap_or_default() },
        outcome: serde_json::from_str(&outcome).unwrap_or_default(),
        quiz,
        questions,
    })
}

pub fn delete(conn: &Connection, id: i64) -> AppResult<()> {
    if conn.execute("DELETE FROM quizzes WHERE id = ?1", [id])? == 0 {
        return Err(AppError::NotFound("小测"));
    }
    Ok(())
}

// --------------------------------------------------------------------------- creating

struct Prepared {
    title: String,
    origin: String,
    section_id: Option<i64>,
    step_id: Option<i64>,
    points: Vec<KnowledgePoint>,
    count: usize,
    kinds: Vec<String>,
    allow_skip: bool,
    mistakes: Vec<String>,
}

fn section_info(conn: &Connection, section_id: i64) -> AppResult<(String, i64, i64)> {
    conn.query_row(
        "SELECT s.title, s.document_id, d.course_id FROM document_sections s JOIN documents d ON d.id = s.document_id WHERE s.id = ?1",
        [section_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )
    .or_not_found("章节")
}

/// Knowledge points of a chapter; created from its headings (or the chapter
/// itself) when none exist, so every question can be linked to a point.
fn points_for_section(conn: &Connection, course_id: i64, section_id: i64) -> AppResult<Vec<KnowledgePoint>> {
    let (title, document_id, course) = section_info(conn, section_id)?;
    if course != course_id {
        return Err(AppError::invalid("章节不属于该课程"));
    }
    let mut points = knowledge::in_section(conn, section_id)?;
    if points.is_empty() {
        knowledge::from_headings(conn, course_id, Some(document_id))?;
        points = knowledge::in_section(conn, section_id)?;
    }
    if points.is_empty() {
        points.push(knowledge::save(conn, KnowledgeInput {
            id: None, course_id, section_id: Some(section_id), title, summary: String::new(), importance: None, page_start: None, page_end: None,
        })?);
    }
    Ok(points)
}

fn prepare(conn: &Connection, input: &CreateQuiz) -> AppResult<Prepared> {
    let quiz_settings = settings::load(conn)?.quiz;
    let mut origin = input.origin.clone().unwrap_or_else(|| "manual".into());
    if !ORIGINS.contains(&origin.as_str()) {
        return Err(AppError::invalid("未知的小测类型"));
    }
    let mut section_id = input.section_id;
    let mut ids = input.knowledge_point_ids.clone();
    let mut mistakes = Vec::new();
    if let Some(step_id) = input.step_id {
        let step = paths::get_step(conn, step_id)?;
        let course: i64 = conn.query_row("SELECT course_id FROM learning_paths WHERE id = ?1", [step.path_id], |r| r.get(0))?;
        if course != input.course_id {
            return Err(AppError::invalid("学习步骤不属于该课程"));
        }
        origin = "step".into();
        section_id = section_id.or(step.section_id);
        if ids.is_empty() {
            ids = step.knowledge_point_ids.clone();
        }
    }
    let today = mastery::today();
    let mut points: Vec<KnowledgePoint> = if !ids.is_empty() {
        let mut found = Vec::new();
        for id in ids.iter().take(20) {
            let point = knowledge::get(conn, *id)?;
            if point.course_id != input.course_id {
                return Err(AppError::invalid("知识点不属于该课程"));
            }
            if !found.iter().any(|p: &KnowledgePoint| p.id == point.id) {
                found.push(point);
            }
        }
        found
    } else if let Some(section) = section_id {
        points_for_section(conn, input.course_id, section)?
    } else if origin == "review" {
        mastery::queue(conn, input.course_id, today)?.items.into_iter().take(8).map(|i| i.point).collect()
    } else if origin == "mistakes" {
        Vec::new()
    } else {
        return Err(AppError::invalid("请选择章节或知识点"));
    };
    if origin == "mistakes" {
        for mistake in list_mistakes(conn, input.course_id, false)?.into_iter().take(10) {
            let answer = match mistake.kind.as_str() {
                "single_choice" => mistake.answer.clone(),
                "true_false" => if mistake.answer == "true" { "正确".into() } else { "错误".into() },
                _ => mistake.answer.chars().take(80).collect(),
            };
            mistakes.push(format!("{}（学生答：{}；参考答案：{}）", mistake.prompt.chars().take(120).collect::<String>(), mistake.response.as_deref().unwrap_or("未作答").chars().take(80).collect::<String>(), answer));
            if let Some(point) = mistake.knowledge_point_id {
                if ids.is_empty() && !points.iter().any(|p| p.id == point) {
                    points.push(knowledge::get(conn, point)?);
                }
            }
        }
    }
    if points.is_empty() {
        return Err(AppError::invalid(match origin.as_str() {
            "review" => "现在没有需要复习的知识点",
            "mistakes" => "错题本里没有未解决的错题",
            _ => "没有可以出题的知识点",
        }));
    }
    let date = today.format("%m-%d").to_string();
    let title = match (origin.as_str(), section_id) {
        ("review", _) => format!("复习小测 · {date}"),
        ("mistakes", _) => format!("错题重练 · {date}"),
        (_, Some(section)) if input.knowledge_point_ids.is_empty() => format!("{} · 小测", section_info(conn, section)?.0),
        _ if points.len() == 1 => format!("{} · 小测", points[0].title),
        _ => format!("{} 等 {} 个知识点 · 小测", points[0].title, points.len()),
    };
    let mut kinds: Vec<String> = input.kinds.clone().unwrap_or(quiz_settings.kinds.clone());
    kinds.retain(|k| settings::QUESTION_KINDS.contains(&k.as_str()));
    kinds.dedup();
    if kinds.is_empty() {
        kinds = quiz_settings.kinds.clone();
    }
    Ok(Prepared {
        title,
        origin,
        section_id,
        step_id: input.step_id,
        points,
        count: input.question_count.unwrap_or(quiz_settings.question_count).clamp(1, 20),
        kinds,
        allow_skip: input.allow_skip.unwrap_or(quiz_settings.allow_skip),
        mistakes,
    })
}

/// Material for the points: their chapters (round-robin so every chapter is
/// represented) plus retrieval for points without a chapter.
fn material(conn: &Connection, course_id: i64, points: &[KnowledgePoint], budget_tokens: usize) -> AppResult<(String, Vec<SourceRef>, bool)> {
    let mut groups: Vec<Vec<ChunkHit>> = Vec::new();
    let mut sections: Vec<i64> = Vec::new();
    for point in points {
        match point.section_id {
            Some(section) if !sections.contains(&section) => {
                sections.push(section);
                groups.push(context::scope_chunks(conn, course_id, &context::ConversationScope { document_ids: vec![], section_ids: vec![section] })?);
            }
            Some(_) => {}
            None => groups.push(search::retrieve(conn, course_id, &format!("{} {}", point.title, point.summary), QueryMode::Any, &Scope::default(), 3)?),
        }
    }
    let total: usize = groups.iter().map(|g| g.len()).sum();
    let mut chosen: Vec<ChunkHit> = Vec::new();
    let mut used = 0;
    let mut round = 0;
    let mut full = false;
    while !full && groups.iter().any(|g| g.len() > round) {
        for group in &groups {
            if let Some(chunk) = group.get(round) {
                if chosen.iter().any(|c| c.chunk_id == chunk.chunk_id) {
                    continue;
                }
                let cost = crate::ai::tokens::estimate(&chunk.content) + 24;
                if used + cost > budget_tokens && !chosen.is_empty() {
                    full = true;
                    break;
                }
                used += cost;
                chosen.push(chunk.clone());
            }
        }
        round += 1;
    }
    let truncated = chosen.len() < total;
    chosen.sort_by_key(|c| (c.document_id, c.chunk_index));
    let (text, sources) = context::render(&chosen, 1);
    Ok((text, sources, truncated))
}

/// Writes a new quiz with the AI (or re-opens the unfinished quiz of a learning step).
pub async fn create(paths: &AppPaths, ai: &AiState, input: CreateQuiz) -> AppResult<QuizDetail> {
    let (prepared, profile, user_prompt, sources, targets) = {
        let conn = paths.connect()?;
        crate::commands::course::find(&conn, input.course_id)?;
        if let Some(step) = input.step_id {
            let open: Option<i64> = conn
                .query_row("SELECT id FROM quizzes WHERE step_id = ?1 AND status != 'graded' ORDER BY id DESC LIMIT 1", [step], |r| r.get(0))
                .optional()?;
            if let Some(id) = open {
                return detail(&conn, id);
            }
        }
        let prepared = prepare(&conn, &input)?;
        let profile = profiles::resolve(&conn, ai.secrets.as_ref(), None)?;
        let reply_budget = (prepared.count * 350 + 600) as u32;
        let budget = profile.context_window.saturating_sub(reply_budget.min(profile.max_output_tokens.max(reply_budget)) as usize + 1500).max(600);
        let (text, sources, _) = material(&conn, input.course_id, &prepared.points, budget)?;
        let targets: Vec<TargetPoint> = prepared.points.iter().map(|p| TargetPoint { id: p.id, title: p.title.clone(), summary: p.summary.clone() }).collect();
        let user_prompt = generate::prompt(&targets, &text, prepared.count, &prepared.kinds, &prepared.mistakes);
        (prepared, profile, user_prompt, sources, targets)
    };
    let wanted = (prepared.count * 350 + 600) as u32;
    let options = ChatOptions { max_tokens: Some(wanted.min(profile.max_output_tokens.max(4096))), temperature: Some(0.4), json: true };
    let messages = [ChatMessage::system(generate::SYSTEM), ChatMessage::user(user_prompt)];
    let completion = ai.gateway.complete(&profile, &messages, &options, &CancellationToken::new()).await?;
    let labels: Vec<String> = sources.iter().map(|s| s.label.clone()).collect();
    let drafts = generate::parse(&completion.text, &targets, &prepared.kinds, &labels, prepared.count).map_err(|e| {
        if completion.finish_reason.as_deref() == Some("length") {
            AppError::msg("AI 的回答长度超出限制被截断。请减少题目数量，或在模型设置中调大“最大输出”。")
        } else {
            AppError::msg(e)
        }
    })?;

    let mut conn = paths.connect()?;
    let usage = completion.usage.unwrap_or_default();
    ai::record_usage(&conn, profile.id, "quiz", usage.prompt_tokens, usage.completion_tokens, completion.usage.is_none())?;
    let id = store(&mut conn, input.course_id, &prepared, &drafts, &sources, &profile.model)?;
    detail(&conn, id)
}

fn store(conn: &mut Connection, course_id: i64, prepared: &Prepared, drafts: &[DraftQuestion], sources: &[SourceRef], model: &str) -> AppResult<i64> {
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO quizzes (course_id, title, origin, section_id, step_id, allow_skip, sources, model) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![course_id, prepared.title, prepared.origin, prepared.section_id, prepared.step_id, prepared.allow_skip, serde_json::to_string(sources)?, model],
    )?;
    let quiz_id = tx.last_insert_rowid();
    for (position, draft) in drafts.iter().enumerate() {
        let cited: Vec<&SourceRef> = sources.iter().filter(|s| draft.sources.contains(&s.label)).collect();
        tx.execute(
            "INSERT INTO quiz_questions (quiz_id, position, kind, knowledge_point_id, prompt, options, answer, explanation, rubric, sources)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                quiz_id, position as i64, draft.kind, draft.point_id, draft.prompt, serde_json::to_string(&draft.options)?, draft.answer,
                draft.explanation, serde_json::to_string(&draft.rubric)?, serde_json::to_string(&cited)?
            ],
        )?;
    }
    tx.commit()?;
    Ok(quiz_id)
}

// --------------------------------------------------------------------------- answering

#[derive(Debug, Clone, Deserialize)]
pub struct AnswerInput {
    pub question_id: i64,
    #[serde(default)]
    pub response: Option<String>,
    #[serde(default)]
    pub skipped: bool,
}

/// Saves one answer (or a skip) while the quiz is open.
pub fn save_answer(conn: &Connection, input: AnswerInput) -> AppResult<Question> {
    let question = get_question(conn, input.question_id)?;
    let quiz = get(conn, question.quiz_id)?;
    if quiz.status != "ready" {
        return Err(AppError::invalid("这份小测已经交卷，不能再修改答案"));
    }
    if input.skipped && !quiz.allow_skip {
        return Err(AppError::invalid("这份小测不允许跳过题目"));
    }
    let response = match input.response.map(|r| r.trim().to_string()).filter(|r| !r.is_empty()) {
        None => None,
        Some(text) => Some(match question.kind.as_str() {
            "single_choice" => {
                let letter = text.to_ascii_uppercase();
                let valid = letter.len() == 1 && (letter.as_bytes()[0].wrapping_sub(b'A') as usize) < question.options.len();
                if !valid {
                    return Err(AppError::invalid("无效的选项"));
                }
                letter
            }
            "true_false" => match text.as_str() {
                "true" | "false" => text,
                _ => return Err(AppError::invalid("判断题只能选择对或错")),
            },
            _ => text.chars().take(MAX_RESPONSE_CHARS).collect(),
        }),
    };
    let skipped = input.skipped && response.is_none();
    conn.execute(
        "UPDATE quiz_questions SET response = ?1, skipped = ?2, answered_at = datetime('now', 'localtime') WHERE id = ?3",
        params![response, skipped, question.id],
    )?;
    conn.execute("UPDATE quizzes SET current_index = ?1 WHERE id = ?2", params![question.position, quiz.id])?;
    let mut saved = get_question(conn, question.id)?;
    saved.answer = None;
    saved.explanation = None;
    saved.rubric.clear();
    saved.sources.clear();
    Ok(saved)
}

/// Remembers which question the student is looking at (restored on reopen).
pub fn set_position(conn: &Connection, quiz_id: i64, index: i64) -> AppResult<()> {
    let quiz = get(conn, quiz_id)?;
    conn.execute("UPDATE quizzes SET current_index = ?1 WHERE id = ?2", params![index.clamp(0, (quiz.question_count - 1).max(0)), quiz_id])?;
    Ok(())
}

// --------------------------------------------------------------------------- grading

/// Hands the quiz in: objective questions are scored at once, written answers
/// are graded by the AI. Unanswered questions count as skipped (score 0).
pub async fn submit(paths: &AppPaths, ai: &AiState, quiz_id: i64) -> AppResult<QuizDetail> {
    {
        let conn = paths.connect()?;
        let quiz = get(&conn, quiz_id)?;
        if quiz.status == "ready" {
            let all = questions(&conn, quiz_id)?;
            let open = all.iter().filter(|q| q.response.is_none() && !q.skipped).count();
            if open > 0 && !quiz.allow_skip {
                return Err(AppError::invalid(format!("还有 {open} 道题没有作答（这份小测不允许跳过）")));
            }
            for question in &all {
                let response = question.response.clone().unwrap_or_default();
                let answer = question.answer.clone().unwrap_or_default();
                let (score, grading, feedback): (Option<f64>, &str, String) = if response.is_empty() {
                    (Some(0.0), "auto", if question.skipped || question.response.is_none() { "未作答（跳过）".into() } else { String::new() })
                } else if generate::is_objective(&question.kind) {
                    (Some(grade::objective(&question.kind, &answer, &response)), "auto", String::new())
                } else {
                    (None, "pending", String::new())
                };
                conn.execute(
                    "UPDATE quiz_questions SET score = ?1, grading = ?2, feedback = ?3, skipped = (skipped OR response IS NULL) WHERE id = ?4",
                    params![score, grading, feedback, question.id],
                )?;
            }
            conn.execute(
                "UPDATE quizzes SET status = 'grading', submitted_at = datetime('now', 'localtime'),
                 pending = (SELECT COUNT(*) FROM quiz_questions WHERE quiz_id = ?1 AND grading = 'pending') WHERE id = ?1",
                [quiz_id],
            )?;
        }
    }
    grade_pending(paths, ai, quiz_id).await
}

/// AI grading of written answers that still have no score.
pub async fn grade_pending(paths: &AppPaths, ai: &AiState, quiz_id: i64) -> AppResult<QuizDetail> {
    let prepared = {
        let conn = paths.connect()?;
        let quiz = get(&conn, quiz_id)?;
        if quiz.status == "ready" {
            return Err(AppError::invalid("请先交卷"));
        }
        let pending: Vec<Question> = questions(&conn, quiz_id)?.into_iter().filter(|q| q.grading == "pending").collect();
        if pending.is_empty() {
            None
        } else {
            match profiles::resolve(&conn, ai.secrets.as_ref(), None) {
                Ok(profile) => Some((profile, pending, grading_material(&conn, quiz_id)?)),
                Err(error) => {
                    conn.execute("UPDATE quizzes SET grading_error = ?1 WHERE id = ?2", params![error.to_string(), quiz_id])?;
                    None
                }
            }
        }
    };
    if let Some((profile, pending, material)) = prepared {
        let labels: Vec<String> = pending.iter().map(|q| format!("Q{}", q.position + 1)).collect();
        let items: Vec<grade::SubjectiveItem<'_>> = pending
            .iter()
            .zip(&labels)
            .map(|(q, label)| grade::SubjectiveItem {
                label: label.clone(),
                kind: &q.kind,
                point: q.knowledge_point_title.as_deref().unwrap_or(""),
                prompt: &q.prompt,
                answer: q.answer.as_deref().unwrap_or(""),
                rubric: &q.rubric,
                response: q.response.as_deref().unwrap_or(""),
            })
            .collect();
        let messages = [ChatMessage::system(grade::SYSTEM), ChatMessage::user(grade::prompt(&items, &material))];
        let options = ChatOptions { max_tokens: Some(((pending.len() * 300 + 300) as u32).min(profile.max_output_tokens.max(2048))), temperature: Some(0.1), json: true };
        let result = ai.gateway.complete(&profile, &messages, &options, &CancellationToken::new()).await;
        let conn = paths.connect()?;
        match result {
            Ok(completion) => {
                let usage = completion.usage.unwrap_or_default();
                ai::record_usage(&conn, profile.id, "grading", usage.prompt_tokens, usage.completion_tokens, completion.usage.is_none())?;
                let graded = grade::parse(&completion.text, &labels);
                for item in &graded {
                    if let Some(question) = pending.iter().zip(&labels).find(|(_, l)| **l == item.label).map(|(q, _)| q) {
                        conn.execute(
                            "UPDATE quiz_questions SET score = ?1, grading = 'ai', feedback = ?2 WHERE id = ?3",
                            params![item.score, item.feedback, question.id],
                        )?;
                    }
                }
                let error = (graded.len() < pending.len()).then_some("AI 没有给出部分题目的评分，可以点击“重新批改”或自己评分");
                conn.execute("UPDATE quizzes SET grading_error = ?1 WHERE id = ?2", params![error, quiz_id])?;
            }
            Err(error) => {
                conn.execute("UPDATE quizzes SET grading_error = ?1 WHERE id = ?2", params![format!("AI 批改失败：{error}"), quiz_id])?;
            }
        }
    }
    let conn = paths.connect()?;
    finalize(&conn, quiz_id)?;
    detail(&conn, quiz_id)
}

/// Material cited by the quiz, with its original labels, for the grader.
fn grading_material(conn: &Connection, quiz_id: i64) -> AppResult<String> {
    let raw: String = conn.query_row("SELECT sources FROM quizzes WHERE id = ?1", [quiz_id], |r| r.get(0))?;
    let sources: Vec<SourceRef> = serde_json::from_str(&raw).unwrap_or_default();
    let mut text = String::new();
    let mut used = 0;
    for source in sources {
        let content: Option<String> = conn.query_row("SELECT content FROM document_chunks WHERE id = ?1", [source.chunk_id], |r| r.get(0)).optional()?;
        let Some(content) = content else { continue };
        used += crate::ai::tokens::estimate(&content);
        if used > 6000 {
            break;
        }
        text.push_str(&format!(
            "<<<资料 {} | {} · {}>>>\n{}\n<<<{} 结束>>>\n\n",
            source.label,
            context::neutralize(&source.filename),
            source.loc_label,
            context::neutralize(content.trim()),
            source.label
        ));
    }
    Ok(text)
}

/// The student grades a written answer themself (after reading the reference answer).
pub fn self_grade(conn: &Connection, question_id: i64, score: f64) -> AppResult<QuizDetail> {
    let question = get_question(conn, question_id)?;
    let quiz = get(conn, question.quiz_id)?;
    if quiz.status == "ready" {
        return Err(AppError::invalid("请先交卷"));
    }
    if generate::is_objective(&question.kind) {
        return Err(AppError::invalid("客观题由系统自动判分"));
    }
    if !score.is_finite() {
        return Err(AppError::invalid("评分无效"));
    }
    if quiz.status == "graded" && question.grading != "pending" {
        return Err(AppError::invalid("这道题已经评分"));
    }
    conn.execute(
        "UPDATE quiz_questions SET score = ?1, grading = 'self', feedback = CASE WHEN feedback = '' THEN '自评' ELSE feedback END WHERE id = ?2",
        params![score.clamp(0.0, 1.0), question_id],
    )?;
    finalize(conn, quiz.id)?;
    detail(conn, quiz.id)
}

/// Once every answer has a score: quiz score, mastery and schedule, mistakes,
/// learning step and history — applied exactly once.
pub fn finalize(conn: &Connection, quiz_id: i64) -> AppResult<()> {
    let quiz = get(conn, quiz_id)?;
    let all = questions(conn, quiz_id)?;
    let pending = all.iter().filter(|q| q.score.is_none()).count() as i64;
    conn.execute("UPDATE quizzes SET pending = ?1 WHERE id = ?2", params![pending, quiz_id])?;
    let applied: bool = conn.query_row("SELECT mastery_applied FROM quizzes WHERE id = ?1", [quiz_id], |r| r.get(0))?;
    if pending > 0 || applied || all.is_empty() {
        return Ok(());
    }
    let total: f64 = all.iter().map(|q| q.score.unwrap_or(0.0)).sum();
    let percent = (total / all.len() as f64 * 1000.0).round() / 10.0;

    // Evidence per knowledge point.
    let mut by_point: Vec<(i64, Vec<f64>)> = Vec::new();
    for question in &all {
        let Some(point) = question.knowledge_point_id else { continue };
        match by_point.iter_mut().find(|(p, _)| *p == point) {
            Some((_, scores)) => scores.push(question.score.unwrap_or(0.0)),
            None => by_point.push((point, vec![question.score.unwrap_or(0.0)])),
        }
    }
    let today = mastery::today();
    let mut outcome = Vec::new();
    for (point, scores) in &by_point {
        let result = scores.iter().sum::<f64>() / scores.len() as f64;
        let evidence = Evidence { result, questions: scores.len() as i64, correct: scores.iter().filter(|s| **s >= PASS).count() as i64 };
        let (before, after) = mastery::record(conn, quiz.course_id, *point, evidence, today)?;
        let current = knowledge::get(conn, *point)?;
        outcome.push(PointOutcome {
            knowledge_point_id: *point,
            title: current.title,
            result,
            before: (before.attempts > 0).then_some(before.score),
            after: after.score,
            status: current.status,
            due_date: after.due_date,
        });
        // Redoing mistakes well clears them.
        if quiz.origin == "mistakes" && result >= mastery::MASTERED {
            conn.execute(
                "UPDATE mistakes SET resolved = 1, resolved_at = datetime('now', 'localtime') WHERE knowledge_point_id = ?1 AND resolved = 0",
                [point],
            )?;
        }
    }
    for question in all.iter().filter(|q| q.score.unwrap_or(0.0) < PASS) {
        conn.execute(
            "INSERT OR IGNORE INTO mistakes (course_id, question_id, knowledge_point_id) VALUES (?1, ?2, ?3)",
            params![quiz.course_id, question.id, question.knowledge_point_id],
        )?;
    }
    conn.execute(
        "UPDATE quizzes SET status = 'graded', score = ?1, mastery_applied = 1, outcome = ?2, graded_at = datetime('now', 'localtime') WHERE id = ?3",
        params![percent, serde_json::to_string(&outcome)?, quiz_id],
    )?;
    study::record(conn, quiz.course_id, "quiz", study::EventRefs { section_id: quiz.section_id, step_id: quiz.step_id, ..Default::default() }, json!({ "quiz_id": quiz_id, "score": percent }))?;
    if let Some(step) = quiz.step_id {
        if paths::get_step(conn, step).map(|s| s.status != "done").unwrap_or(false) {
            paths::set_status(conn, step, "done", Some(&format!("小测得分 {percent}%")))?;
        }
    }
    Ok(())
}

// --------------------------------------------------------------------------- mistake book

#[derive(Debug, Clone, Serialize)]
pub struct Mistake {
    pub id: i64,
    pub course_id: i64,
    pub question_id: i64,
    pub quiz_id: i64,
    pub quiz_title: String,
    pub knowledge_point_id: Option<i64>,
    pub knowledge_point_title: Option<String>,
    pub kind: String,
    pub kind_label: String,
    pub prompt: String,
    pub options: Vec<String>,
    pub response: Option<String>,
    pub answer: String,
    pub explanation: String,
    pub feedback: String,
    pub score: Option<f64>,
    pub sources: Vec<SourceRef>,
    pub resolved: bool,
    pub created_at: String,
}

pub fn list_mistakes(conn: &Connection, course_id: i64, include_resolved: bool) -> AppResult<Vec<Mistake>> {
    let mut stmt = conn.prepare(
        "SELECT m.id, m.course_id, m.question_id, x.quiz_id, q.title, m.knowledge_point_id, k.title, x.kind, x.prompt, x.options, x.response,
                x.answer, x.explanation, x.feedback, x.score, x.sources, m.resolved, m.created_at
         FROM mistakes m JOIN quiz_questions x ON x.id = m.question_id JOIN quizzes q ON q.id = x.quiz_id
         LEFT JOIN knowledge_points k ON k.id = m.knowledge_point_id
         WHERE m.course_id = ?1 AND (?2 OR m.resolved = 0) ORDER BY m.resolved, m.created_at DESC, m.id DESC",
    )?;
    let rows = stmt.query_map(params![course_id, include_resolved], |r| {
        let kind: String = r.get(7)?;
        Ok(Mistake {
            id: r.get(0)?,
            course_id: r.get(1)?,
            question_id: r.get(2)?,
            quiz_id: r.get(3)?,
            quiz_title: r.get(4)?,
            knowledge_point_id: r.get(5)?,
            knowledge_point_title: r.get(6)?,
            kind_label: generate::kind_label(&kind).to_string(),
            kind,
            prompt: r.get(8)?,
            options: serde_json::from_str(&r.get::<_, String>(9)?).unwrap_or_default(),
            response: r.get(10)?,
            answer: r.get(11)?,
            explanation: r.get(12)?,
            feedback: r.get(13)?,
            score: r.get(14)?,
            sources: serde_json::from_str(&r.get::<_, String>(15)?).unwrap_or_default(),
            resolved: r.get(16)?,
            created_at: r.get(17)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn resolve_mistake(conn: &Connection, id: i64, resolved: bool) -> AppResult<()> {
    let changed = conn.execute(
        "UPDATE mistakes SET resolved = ?1, resolved_at = CASE WHEN ?1 THEN datetime('now', 'localtime') ELSE NULL END WHERE id = ?2",
        params![resolved, id],
    )?;
    if changed == 0 {
        return Err(AppError::NotFound("错题"));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
