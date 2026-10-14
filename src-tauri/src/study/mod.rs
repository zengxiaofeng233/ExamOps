//! Study State Engine: where the student is, what has been done, and a
//! learning-history log. "学过" (studied) and "已掌握" (mastered) are separate
//! states on knowledge points; mastery is only granted by quiz results.

use crate::error::AppResult;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Default, Clone)]
pub struct EventRefs {
    pub knowledge_point_id: Option<i64>,
    pub step_id: Option<i64>,
    pub section_id: Option<i64>,
    pub document_id: Option<i64>,
}

pub fn record(conn: &Connection, course_id: i64, kind: &str, refs: EventRefs, detail: Value) -> AppResult<()> {
    conn.execute(
        "INSERT INTO study_events (course_id, kind, knowledge_point_id, step_id, section_id, document_id, detail) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![course_id, kind, refs.knowledge_point_id, refs.step_id, refs.section_id, refs.document_id, detail.to_string()],
    )?;
    conn.execute("UPDATE courses SET updated_at = datetime('now', 'localtime') WHERE id = ?1", [course_id])?;
    Ok(())
}

fn ensure(conn: &Connection, course_id: i64) -> AppResult<()> {
    conn.execute("INSERT OR IGNORE INTO study_state (course_id) VALUES (?1)", [course_id])?;
    Ok(())
}

pub fn set_active_path(conn: &Connection, course_id: i64, path_id: Option<i64>) -> AppResult<()> {
    ensure(conn, course_id)?;
    conn.execute(
        "UPDATE study_state SET active_path_id = ?1, updated_at = datetime('now', 'localtime') WHERE course_id = ?2",
        params![path_id, course_id],
    )?;
    Ok(())
}

pub fn set_current_step(conn: &Connection, course_id: i64, path_id: i64, step_id: i64, section_id: Option<i64>) -> AppResult<()> {
    ensure(conn, course_id)?;
    conn.execute(
        "UPDATE study_state SET active_path_id = ?1, current_step_id = ?2, current_section_id = COALESCE(?3, current_section_id),
         updated_at = datetime('now', 'localtime') WHERE course_id = ?4",
        params![path_id, step_id, section_id, course_id],
    )?;
    Ok(())
}

pub fn set_reading(conn: &Connection, course_id: i64, document_id: i64, page: i64) -> AppResult<()> {
    ensure(conn, course_id)?;
    let section: Option<i64> = conn
        .query_row(
            "SELECT id FROM document_sections WHERE document_id = ?1 AND page_start <= ?2 AND page_end >= ?2 ORDER BY level DESC LIMIT 1",
            params![document_id, page],
            |r| r.get(0),
        )
        .optional()?;
    conn.execute(
        "UPDATE study_state SET last_document_id = ?1, last_page = ?2, current_section_id = COALESCE(?3, current_section_id),
         updated_at = datetime('now', 'localtime') WHERE course_id = ?4",
        params![document_id, page, section, course_id],
    )?;
    Ok(())
}

/// Saves the reader position and logs a "read" event at most every 10 minutes per document.
pub fn note_reading(conn: &Connection, course_id: i64, document_id: i64, page: i64) -> AppResult<()> {
    let course: i64 = conn
        .query_row("SELECT course_id FROM documents WHERE id = ?1", [document_id], |r| r.get(0))
        .optional()?
        .ok_or(crate::error::AppError::NotFound("资料"))?;
    if course != course_id {
        return Err(crate::error::AppError::invalid("资料不属于该课程"));
    }
    set_reading(conn, course_id, document_id, page)?;
    let recent: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM study_events WHERE course_id = ?1 AND kind = 'read' AND document_id = ?2
         AND created_at > datetime('now', 'localtime', '-10 minutes'))",
        params![course_id, document_id],
        |r| r.get(0),
    )?;
    if !recent {
        let section: Option<i64> = conn.query_row("SELECT current_section_id FROM study_state WHERE course_id = ?1", [course_id], |r| r.get(0))?;
        record(conn, course_id, "read", EventRefs { document_id: Some(document_id), section_id: section, ..Default::default() }, serde_json::json!({ "page": page }))?;
    }
    Ok(())
}

pub fn set_conversation(conn: &Connection, course_id: i64, conversation_id: i64, section_id: Option<i64>) -> AppResult<()> {
    ensure(conn, course_id)?;
    conn.execute(
        "UPDATE study_state SET last_conversation_id = ?1, current_section_id = COALESCE(?2, current_section_id),
         updated_at = datetime('now', 'localtime') WHERE course_id = ?3",
        params![conversation_id, section_id, course_id],
    )?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Counts {
    pub documents: i64,
    pub sections: i64,
    pub points: i64,
    pub points_learned: i64,
    pub points_mastered: i64,
    pub points_difficult: i64,
    pub steps: i64,
    pub steps_done: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct StudyState {
    pub course_id: i64,
    pub active_path: Option<crate::paths::LearningPath>,
    pub current_step: Option<crate::paths::PathStep>,
    pub current_section_id: Option<i64>,
    pub current_section_title: Option<String>,
    pub last_document_id: Option<i64>,
    pub last_document_name: Option<String>,
    pub last_page: Option<i64>,
    pub last_conversation: Option<crate::tutor::Conversation>,
    pub counts: Counts,
    pub activity: Vec<DayActivity>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DayActivity {
    pub date: String,
    pub events: i64,
}

pub fn counts(conn: &Connection, course_id: i64) -> AppResult<Counts> {
    Ok(conn.query_row(
        "SELECT
            (SELECT COUNT(*) FROM documents WHERE course_id = ?1 AND status = 'completed'),
            (SELECT COUNT(*) FROM document_sections s JOIN documents d ON d.id = s.document_id WHERE d.course_id = ?1 AND s.level = 1),
            (SELECT COUNT(*) FROM knowledge_points WHERE course_id = ?1),
            (SELECT COUNT(*) FROM knowledge_points WHERE course_id = ?1 AND status IN ('learned', 'mastered')),
            (SELECT COUNT(*) FROM knowledge_points WHERE course_id = ?1 AND status = 'mastered'),
            (SELECT COUNT(*) FROM knowledge_points WHERE course_id = ?1 AND difficult = 1),
            (SELECT COUNT(*) FROM path_steps st JOIN learning_paths p ON p.id = st.path_id WHERE p.course_id = ?1 AND p.archived = 0),
            (SELECT COUNT(*) FROM path_steps st JOIN learning_paths p ON p.id = st.path_id WHERE p.course_id = ?1 AND p.archived = 0 AND st.status = 'done')",
        [course_id],
        |r| Ok(Counts {
            documents: r.get(0)?,
            sections: r.get(1)?,
            points: r.get(2)?,
            points_learned: r.get(3)?,
            points_mastered: r.get(4)?,
            points_difficult: r.get(5)?,
            steps: r.get(6)?,
            steps_done: r.get(7)?,
        }),
    )?)
}

/// Events per day for the last `days` days (oldest first, gaps filled with 0).
pub fn activity(conn: &Connection, course_id: i64, days: i64) -> AppResult<Vec<DayActivity>> {
    let mut stmt = conn.prepare(
        "SELECT date(created_at), COUNT(*) FROM study_events WHERE course_id = ?1 AND date(created_at) > date('now', 'localtime', ?2)
         GROUP BY date(created_at)",
    )?;
    let rows = stmt.query_map(params![course_id, format!("-{days} days")], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
    let found: std::collections::HashMap<String, i64> = rows.collect::<rusqlite::Result<_>>()?;
    let today = chrono::Local::now().date_naive();
    Ok((0..days)
        .rev()
        .map(|offset| {
            let date = (today - chrono::Duration::days(offset)).format("%Y-%m-%d").to_string();
            DayActivity { events: *found.get(&date).unwrap_or(&0), date }
        })
        .collect())
}

/// Raw `study_state` row: path, step, section, document, page, conversation, updated.
type StateRow = (Option<i64>, Option<i64>, Option<i64>, Option<i64>, Option<i64>, Option<i64>, String);

pub fn state(conn: &Connection, course_id: i64) -> AppResult<StudyState> {
    let row: Option<StateRow> = conn
        .query_row(
            "SELECT active_path_id, current_step_id, current_section_id, last_document_id, last_page, last_conversation_id, updated_at
             FROM study_state WHERE course_id = ?1",
            [course_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)),
        )
        .optional()?;
    let (path_id, step_id, section_id, document_id, page, conversation_id, updated_at) = match row {
        Some((a, b, c, d, e, f, g)) => (a, b, c, d, e, f, Some(g)),
        None => (None, None, None, None, None, None, None),
    };
    // Fall back to the most recently used path so a returning student always has a next step.
    let path_id = match path_id {
        Some(id) => Some(id),
        None => conn
            .query_row("SELECT id FROM learning_paths WHERE course_id = ?1 AND archived = 0 ORDER BY updated_at DESC LIMIT 1", [course_id], |r| r.get(0))
            .optional()?,
    };
    let active_path = match path_id {
        Some(id) => crate::paths::get_path(conn, id).ok(),
        None => None,
    };
    let current_step = match (&active_path, step_id) {
        (Some(path), Some(step)) => crate::paths::get_step(conn, step).ok().filter(|s| s.path_id == path.id && s.status != "done" && s.status != "skipped")
            .or_else(|| crate::paths::next_step(conn, path.id).ok().flatten()),
        (Some(path), None) => crate::paths::next_step(conn, path.id)?,
        _ => None,
    };
    let current_section_title: Option<String> = match section_id {
        Some(id) => conn.query_row("SELECT title FROM document_sections WHERE id = ?1", [id], |r| r.get(0)).optional()?,
        None => None,
    };
    let last_document_name: Option<String> = match document_id {
        Some(id) => conn.query_row("SELECT filename FROM documents WHERE id = ?1", [id], |r| r.get(0)).optional()?,
        None => None,
    };
    let last_conversation = match conversation_id {
        Some(id) => crate::tutor::get(conn, id).ok(),
        None => None,
    };
    Ok(StudyState {
        course_id,
        active_path,
        current_step,
        current_section_id: section_id,
        current_section_title,
        last_document_id: document_id,
        last_document_name,
        last_page: page,
        last_conversation,
        counts: counts(conn, course_id)?,
        activity: activity(conn, course_id, 14)?,
        updated_at,
    })
}

/// One course on the home page: progress, reviews due and the next step.
#[derive(Debug, Clone, Serialize)]
pub struct CourseDigest {
    pub course_id: i64,
    pub name: String,
    pub exam_date: Option<String>,
    pub days_to_exam: Option<i64>,
    pub points: i64,
    pub learned: i64,
    pub mastered: i64,
    pub due: usize,
    pub weak: usize,
    pub open_mistakes: i64,
    pub next_step: Option<String>,
    pub updated_at: String,
}

pub fn dashboard(conn: &Connection, today: chrono::NaiveDate) -> AppResult<Vec<CourseDigest>> {
    let courses: Vec<(i64, String, String)> = {
        let mut stmt = conn.prepare("SELECT id, name, updated_at FROM courses ORDER BY updated_at DESC, id DESC")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    let mut out = Vec::new();
    for (id, name, updated_at) in courses {
        let queue = crate::mastery::queue(conn, id, today)?;
        let state = state(conn, id)?;
        out.push(CourseDigest {
            course_id: id,
            name,
            exam_date: queue.summary.exam_date.clone(),
            days_to_exam: queue.summary.days_to_exam,
            points: state.counts.points,
            learned: state.counts.points_learned,
            mastered: state.counts.points_mastered,
            due: queue.summary.due,
            weak: queue.summary.weak,
            open_mistakes: queue.summary.open_mistakes,
            next_step: state.current_step.map(|s| s.title),
            updated_at,
        });
    }
    Ok(out)
}

/// Compact course map given to the AI when it plans a path.
pub fn catalogue(conn: &Connection, course_id: i64) -> AppResult<String> {
    let mut out = String::from("【资料与章节】（[id] 标题 · 位置）\n");
    let documents = crate::documents::list(conn, course_id)?;
    let mut any = false;
    for document in documents.iter().rev().filter(|d| d.status == "completed") {
        any = true;
        let kind = if document.file_type == "pptx" { "PPT" } else { "教材/讲义" };
        out.push_str(&format!("《{}》（{kind}）\n", document.filename));
        for section in crate::documents::sections(conn, document.id)?.into_iter().filter(|s| s.level <= 2).take(80) {
            out.push_str(&format!("{}[{}] {} · {}\n", "  ".repeat(section.level as usize), section.id, section.title, section.loc_label));
        }
    }
    if !any {
        return Ok(String::new());
    }
    let videos = crate::resources::list(conn, course_id)?;
    if !videos.is_empty() {
        out.push_str("【网课】\n");
        for video in videos {
            let usable = !video.transcript.trim().is_empty() || !video.notes.trim().is_empty();
            out.push_str(&format!(
                "  [{}] {}（关联章节：{}；有字幕或笔记：{}）\n",
                video.id,
                video.title,
                video.section_title.as_deref().unwrap_or("无"),
                if usable { "是" } else { "否" }
            ));
        }
    }
    let points = crate::knowledge::list(conn, course_id)?;
    if !points.is_empty() {
        out.push_str("【知识点】（[id] 名称 · 状态）\n");
        for point in points.into_iter().take(150) {
            let status = match point.status.as_str() {
                "mastered" => "已掌握",
                "learned" => "学过",
                _ => "未学",
            };
            out.push_str(&format!("  [{}] {} · {status}{}\n", point.id, point.title, if point.difficult { " · 疑难" } else { "" }));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support;

    #[test]
    fn state_tracks_reading_steps_and_activity() {
        let (paths, mut conn) = test_support::course_with_document();
        let empty = state(&conn, 1).unwrap();
        assert!(empty.active_path.is_none() && empty.updated_at.is_none());
        note_reading(&conn, 1, 1, 3).unwrap();
        note_reading(&conn, 1, 1, 2).unwrap();
        assert!(note_reading(&conn, 2, 1, 1).is_err());
        let reading = state(&conn, 1).unwrap();
        assert_eq!((reading.last_document_id, reading.last_page), (Some(1), Some(2)));
        assert_eq!(reading.current_section_title.as_deref(), Some("1.1 极限的定义"));
        // Page turns within ten minutes count as one reading session.
        assert_eq!(reading.activity.last().unwrap().events, 1);
        let detail = crate::paths::from_template(&mut conn, crate::paths::TemplateInput { course_id: 1, name: None, mode: None, document_ids: vec![], include_quiz: false }).unwrap();
        let first = &detail.steps[0];
        let after = state(&conn, 1).unwrap();
        assert_eq!(after.active_path.as_ref().unwrap().id, detail.path.id);
        assert_eq!(after.current_step.as_ref().unwrap().id, first.id);
        crate::paths::set_status(&conn, first.id, "done", None).unwrap();
        let moved = state(&conn, 1).unwrap();
        assert_eq!(moved.current_step.unwrap().id, detail.steps[1].id);
        assert_eq!(moved.activity.len(), 14);
        assert_eq!(moved.activity.last().unwrap().events, 2);
        assert_eq!(moved.counts.steps_done, 1);
        let catalogue = catalogue(&conn, 1).unwrap();
        assert!(catalogue.contains("第二章 导数") && catalogue.contains("《高等数学讲义.pdf》"));
        let digest = dashboard(&conn, chrono::Local::now().date_naive()).unwrap();
        assert_eq!(digest.len(), 2);
        let first = digest.iter().find(|d| d.course_id == 1).unwrap();
        assert_eq!(first.next_step.as_deref(), Some(detail.steps[1].title.as_str()));
        drop(conn);
        test_support::cleanup(paths);
    }
}
