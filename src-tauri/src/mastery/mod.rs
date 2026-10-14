//! Knowledge mastery and spaced review (Roadmap · Study State Engine).
//!
//! * Mastery is an exponential moving average of quiz results per knowledge
//!   point. "已掌握" needs a high average over at least two answered questions;
//!   a later poor result takes the point back to "学过". Studying (reading,
//!   lessons, videos) never grants mastery.
//! * Review dates follow an SM-2 style schedule: 1 day → 3 days → interval ×
//!   ease, back to tomorrow after a poor result, and compressed so that at
//!   least one more review happens before the exam date.

use crate::error::AppResult;
use crate::knowledge::{self, KnowledgePoint};
use chrono::{Duration, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

/// Average needed for "已掌握".
pub const MASTERED: f64 = 0.8;
/// Below this a tested point counts as weak.
pub const WEAK: f64 = 0.6;
/// A question scored at least this much counts as correct.
pub const PASS: f64 = 0.6;
const EMA_WEIGHT: f64 = 0.5;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Mastery {
    pub knowledge_point_id: i64,
    pub score: f64,
    pub attempts: i64,
    pub correct: i64,
    pub streak: i64,
    pub ease: f64,
    pub interval_days: i64,
    pub due_date: Option<String>,
    pub last_result: Option<f64>,
    pub last_quiz_at: Option<String>,
}

impl Mastery {
    fn fresh(point: i64) -> Mastery {
        Mastery {
            knowledge_point_id: point, score: 0.0, attempts: 0, correct: 0, streak: 0, ease: 2.5, interval_days: 0,
            due_date: None, last_result: None, last_quiz_at: None,
        }
    }
}

/// One quiz's evidence about a knowledge point.
#[derive(Debug, Clone, Copy)]
pub struct Evidence {
    /// Average question score (0..1).
    pub result: f64,
    pub questions: i64,
    pub correct: i64,
}

/// Pure update: new mastery and schedule after a quiz.
pub fn apply(previous: &Mastery, evidence: Evidence, today: NaiveDate, exam: Option<NaiveDate>) -> Mastery {
    let result = evidence.result.clamp(0.0, 1.0);
    let mut next = previous.clone();
    next.score = if previous.attempts == 0 { result } else { previous.score * (1.0 - EMA_WEIGHT) + result * EMA_WEIGHT };
    next.attempts += evidence.questions;
    next.correct += evidence.correct;
    next.last_result = Some(result);

    // SM-2: quality 0–5 from the result.
    let quality = (result * 5.0).round();
    next.ease = (previous.ease + 0.1 - (5.0 - quality) * (0.08 + (5.0 - quality) * 0.02)).max(1.3);
    if quality >= 3.0 {
        next.streak += 1;
        next.interval_days = match next.streak {
            1 => 1,
            2 => 3,
            _ => ((previous.interval_days.max(1) as f64) * next.ease).round() as i64,
        }
        .max(previous.interval_days + 1)
        .min(180);
    } else {
        next.streak = 0;
        next.interval_days = 1;
    }
    if let Some(exam) = exam {
        let left = (exam - today).num_days();
        if left <= 1 {
            next.interval_days = 1;
        } else if next.interval_days > left / 2 {
            // Keep at least one more review before the exam.
            next.interval_days = (left / 2).max(1);
        }
    }
    next.due_date = Some((today + Duration::days(next.interval_days)).format("%Y-%m-%d").to_string());
    next
}

pub fn is_mastered(mastery: &Mastery) -> bool {
    mastery.score >= MASTERED && mastery.attempts >= 2
}

fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Mastery> {
    Ok(Mastery {
        knowledge_point_id: row.get(0)?,
        score: row.get(1)?,
        attempts: row.get(2)?,
        correct: row.get(3)?,
        streak: row.get(4)?,
        ease: row.get(5)?,
        interval_days: row.get(6)?,
        due_date: row.get(7)?,
        last_result: row.get(8)?,
        last_quiz_at: row.get(9)?,
    })
}

const COLUMNS: &str = "knowledge_point_id, score, attempts, correct, streak, ease, interval_days, due_date, last_result, last_quiz_at";

pub fn get(conn: &Connection, point: i64) -> AppResult<Option<Mastery>> {
    Ok(conn.query_row(&format!("SELECT {COLUMNS} FROM mastery WHERE knowledge_point_id = ?1"), [point], from_row).optional()?)
}

pub fn list(conn: &Connection, course_id: i64) -> AppResult<Vec<Mastery>> {
    let mut stmt = conn.prepare(&format!("SELECT {COLUMNS} FROM mastery WHERE course_id = ?1"))?;
    let rows = stmt.query_map([course_id], from_row)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn today() -> NaiveDate {
    chrono::Local::now().date_naive()
}

pub fn exam_date(conn: &Connection, course_id: i64) -> AppResult<Option<NaiveDate>> {
    let date: Option<String> = conn.query_row("SELECT exam_date FROM courses WHERE id = ?1", [course_id], |r| r.get(0)).optional()?.flatten();
    Ok(date.and_then(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()))
}

/// Stores the new mastery of a point and moves its status between 学过 and 已掌握.
pub fn record(conn: &Connection, course_id: i64, point: i64, evidence: Evidence, today: NaiveDate) -> AppResult<(Mastery, Mastery)> {
    let previous = get(conn, point)?.unwrap_or_else(|| Mastery::fresh(point));
    let next = apply(&previous, evidence, today, exam_date(conn, course_id)?);
    conn.execute(
        "INSERT INTO mastery (knowledge_point_id, course_id, score, attempts, correct, streak, ease, interval_days, due_date, last_result, last_quiz_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, datetime('now', 'localtime'), datetime('now', 'localtime'))
         ON CONFLICT(knowledge_point_id) DO UPDATE SET score = excluded.score, attempts = excluded.attempts, correct = excluded.correct,
            streak = excluded.streak, ease = excluded.ease, interval_days = excluded.interval_days, due_date = excluded.due_date,
            last_result = excluded.last_result, last_quiz_at = excluded.last_quiz_at, updated_at = excluded.updated_at",
        params![point, course_id, next.score, next.attempts, next.correct, next.streak, next.ease, next.interval_days, next.due_date, next.last_result],
    )?;
    let status = if is_mastered(&next) { "mastered" } else { "learned" };
    conn.execute(
        "UPDATE knowledge_points SET status = ?1, learned_at = COALESCE(learned_at, datetime('now', 'localtime')),
         updated_at = datetime('now', 'localtime') WHERE id = ?2",
        params![status, point],
    )?;
    let stored = get(conn, point)?.unwrap_or(next);
    Ok((previous, stored))
}

// --------------------------------------------------------------------------- review queue

#[derive(Debug, Clone, Serialize)]
pub struct ReviewItem {
    pub point: KnowledgePoint,
    pub mastery: Option<Mastery>,
    /// due | weak | difficult | untested
    pub reason: String,
    pub reason_label: String,
    pub priority: f64,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ReviewSummary {
    pub due: usize,
    pub overdue: usize,
    pub weak: usize,
    pub untested: usize,
    pub open_mistakes: i64,
    pub mastered: usize,
    pub tested: usize,
    pub average: Option<f64>,
    pub next_due: Option<String>,
    pub exam_date: Option<String>,
    pub days_to_exam: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReviewQueue {
    pub summary: ReviewSummary,
    pub items: Vec<ReviewItem>,
}

/// What to review now: due points first, then weak, difficult and studied-but-untested ones.
pub fn queue(conn: &Connection, course_id: i64, today: NaiveDate) -> AppResult<ReviewQueue> {
    let points = knowledge::list(conn, course_id)?;
    let masteries: std::collections::HashMap<i64, Mastery> = list(conn, course_id)?.into_iter().map(|m| (m.knowledge_point_id, m)).collect();
    let open_mistakes: std::collections::HashMap<i64, i64> = {
        let mut stmt = conn.prepare("SELECT knowledge_point_id, COUNT(*) FROM mistakes WHERE course_id = ?1 AND resolved = 0 AND knowledge_point_id IS NOT NULL GROUP BY knowledge_point_id")?;
        let rows = stmt.query_map([course_id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    let exam = exam_date(conn, course_id)?;
    let mut summary = ReviewSummary {
        open_mistakes: conn.query_row("SELECT COUNT(*) FROM mistakes WHERE course_id = ?1 AND resolved = 0", [course_id], |r| r.get(0))?,
        exam_date: exam.map(|d| d.format("%Y-%m-%d").to_string()),
        days_to_exam: exam.map(|d| (d - today).num_days()),
        ..Default::default()
    };
    let mut items = Vec::new();
    let mut total = 0.0;
    for point in points {
        let mastery = masteries.get(&point.id).cloned();
        let importance = point.importance as f64;
        let mistakes = *open_mistakes.get(&point.id).unwrap_or(&0) as f64;
        let entry = match &mastery {
            Some(m) => {
                summary.tested += 1;
                total += m.score;
                if is_mastered(m) {
                    summary.mastered += 1;
                }
                let due = m.due_date.as_deref().and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());
                if let Some(due) = due {
                    if summary.next_due.as_deref().map(|n| due.format("%Y-%m-%d").to_string().as_str() < n).unwrap_or(true) && due > today {
                        summary.next_due = Some(due.format("%Y-%m-%d").to_string());
                    }
                }
                match due {
                    Some(due) if due <= today => {
                        let late = (today - due).num_days();
                        summary.due += 1;
                        if late > 0 {
                            summary.overdue += 1;
                        }
                        let label = if late > 0 { format!("已过期 {late} 天") } else { "今天到期".to_string() };
                        Some(("due", label, 6.0 + late.min(14) as f64 * 0.5 + (1.0 - m.score) * 4.0 + importance + mistakes))
                    }
                    _ if m.score < WEAK => {
                        summary.weak += 1;
                        Some(("weak", format!("掌握度 {}%", (m.score * 100.0).round()), 4.0 + (1.0 - m.score) * 4.0 + importance + mistakes))
                    }
                    _ if point.difficult && !is_mastered(m) => Some(("difficult", "标记为疑难".to_string(), 3.0 + importance + mistakes)),
                    _ => None,
                }
            }
            None if point.difficult => Some(("difficult", "标记为疑难，尚未测验".to_string(), 3.5 + importance)),
            None if point.status == "learned" => {
                summary.untested += 1;
                Some(("untested", "学过但还没测验".to_string(), 2.0 + importance))
            }
            None => None,
        };
        if let Some((reason, label, priority)) = entry {
            items.push(ReviewItem { point, mastery, reason: reason.into(), reason_label: label, priority });
        }
    }
    summary.average = (summary.tested > 0).then(|| total / summary.tested as f64);
    items.sort_by(|a, b| b.priority.total_cmp(&a.priority).then(a.point.id.cmp(&b.point.id)));
    Ok(ReviewQueue { summary, items })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support;

    fn day(text: &str) -> NaiveDate {
        NaiveDate::parse_from_str(text, "%Y-%m-%d").unwrap()
    }

    fn evidence(result: f64) -> Evidence {
        Evidence { result, questions: 2, correct: if result >= PASS { 2 } else { 0 } }
    }

    #[test]
    fn schedule_grows_with_success_and_resets_after_failure() {
        let today = day("2026-10-14");
        let first = apply(&Mastery::fresh(1), evidence(1.0), today, None);
        assert_eq!((first.score, first.streak, first.interval_days, first.due_date.as_deref()), (1.0, 1, 1, Some("2026-10-15")));
        let second = apply(&first, evidence(1.0), day("2026-10-15"), None);
        assert_eq!((second.streak, second.interval_days), (2, 3));
        assert!(is_mastered(&second));
        // A barely passing result still extends the interval but lowers the ease.
        let third = apply(&second, evidence(0.6), day("2026-10-18"), None);
        assert!(third.interval_days > 3 && third.ease < second.ease);
        // A poor result: back to tomorrow, average drops, no longer mastered.
        let failed = apply(&third, evidence(0.2), day("2026-10-25"), None);
        assert_eq!((failed.streak, failed.interval_days), (0, 1));
        assert!(!is_mastered(&failed) && failed.score < 0.65);
        assert!(failed.ease >= 1.3);
    }

    #[test]
    fn mastery_needs_more_than_one_lucky_answer_and_exams_compress_intervals() {
        let today = day("2026-10-14");
        let once = apply(&Mastery::fresh(1), Evidence { result: 1.0, questions: 1, correct: 1 }, today, None);
        assert!(!is_mastered(&once));
        let mut long = Mastery { streak: 5, interval_days: 30, ease: 2.6, attempts: 8, score: 0.9, ..Mastery::fresh(1) };
        long = apply(&long, evidence(1.0), today, Some(day("2026-10-24")));
        assert_eq!(long.interval_days, 5); // half of the 10 days left
        let eve = apply(&long, evidence(1.0), day("2026-10-23"), Some(day("2026-10-24")));
        assert_eq!(eve.interval_days, 1);
    }

    #[test]
    fn records_update_status_and_the_review_queue() {
        let (paths, conn) = test_support::course_with_document();
        knowledge::from_headings(&conn, 1, None).unwrap();
        let points = knowledge::list(&conn, 1).unwrap();
        let (limit, derivative) = (points[0].id, points[1].id);
        knowledge::mark_learned(&conn, &[derivative]).unwrap();
        let today = day("2026-10-14");
        let empty = queue(&conn, 1, today).unwrap();
        assert_eq!(empty.summary.untested, 1);
        assert_eq!(empty.items[0].reason, "untested");

        record(&conn, 1, limit, evidence(0.3), today).unwrap();
        assert_eq!(knowledge::get(&conn, limit).unwrap().status, "learned");
        let (_, good) = record(&conn, 1, derivative, evidence(1.0), today).unwrap();
        record(&conn, 1, derivative, evidence(1.0), today).unwrap();
        assert_eq!(knowledge::get(&conn, derivative).unwrap().status, "mastered");
        assert_eq!(good.due_date.as_deref(), Some("2026-10-15"));

        // Tomorrow the failed point is due; the mastered one waits until its next date.
        let later = queue(&conn, 1, day("2026-10-15")).unwrap();
        assert_eq!((later.summary.tested, later.summary.mastered, later.summary.due), (2, 1, 1));
        assert_eq!(later.summary.next_due.as_deref(), Some("2026-10-17"));
        assert_eq!(later.items.iter().map(|i| (i.point.id, i.reason.as_str())).collect::<Vec<_>>(), vec![(limit, "due")]);
        // Two days late it is overdue and ranked by lateness and weakness.
        let late = queue(&conn, 1, day("2026-10-17")).unwrap();
        assert_eq!((late.summary.due, late.summary.overdue), (2, 1));
        assert_eq!(late.items[0].point.id, limit);
        assert_eq!(late.items[0].reason_label, "已过期 2 天");
        drop(conn);
        test_support::cleanup(paths);
    }
}
