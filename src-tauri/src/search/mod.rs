//! Keyword search over a course's materials (SQLite FTS5 + BM25).

pub mod segment;

use crate::error::AppResult;
use rusqlite::{params_from_iter, types::Value, Connection};
use segment::{QueryMode, Segment};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct SearchHit {
    pub chunk_id: i64,
    pub document_id: i64,
    pub filename: String,
    pub file_type: String,
    pub section_id: Option<i64>,
    pub section_title: Option<String>,
    pub page_start: i64,
    pub page_end: i64,
    pub loc_label: String,
    pub snippet: Vec<Segment>,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    pub query: String,
    pub terms: Vec<String>,
    pub hits: Vec<SearchHit>,
}

/// A retrieved chunk with its full text (used by the Context Engine).
#[derive(Debug, Clone, Serialize)]
pub struct ChunkHit {
    pub chunk_id: i64,
    pub document_id: i64,
    pub filename: String,
    pub section_id: Option<i64>,
    pub section_title: Option<String>,
    pub chunk_index: i64,
    pub page_start: i64,
    pub page_end: i64,
    pub loc_label: String,
    pub content: String,
    pub score: f64,
}

/// Restricts a search to part of a course.
#[derive(Debug, Clone, Default)]
pub struct Scope {
    pub document_ids: Vec<i64>,
    pub section_ids: Vec<i64>,
}

pub(crate) fn scope_sql(scope: &Scope, args: &mut Vec<Value>) -> String {
    let mut sql = String::new();
    if !scope.document_ids.is_empty() {
        sql.push_str(&format!(" AND c.document_id IN ({})", vec!["?"; scope.document_ids.len()].join(",")));
        args.extend(scope.document_ids.iter().map(|id| Value::Integer(*id)));
    }
    if !scope.section_ids.is_empty() {
        // A section scope includes its sub-sections.
        sql.push_str(&format!(
            " AND c.section_id IN (WITH RECURSIVE tree(id) AS (SELECT id FROM document_sections WHERE id IN ({})
               UNION SELECT s.id FROM document_sections s JOIN tree ON s.parent_id = tree.id) SELECT id FROM tree)",
            vec!["?"; scope.section_ids.len()].join(",")
        ));
        args.extend(scope.section_ids.iter().map(|id| Value::Integer(*id)));
    }
    sql
}

pub fn retrieve(conn: &Connection, course_id: i64, query: &str, mode: QueryMode, scope: &Scope, limit: usize) -> AppResult<Vec<ChunkHit>> {
    let Some(expression) = segment::fts_query(query, mode) else {
        return Ok(Vec::new());
    };
    let mut args: Vec<Value> = vec![Value::Text(expression), Value::Integer(course_id)];
    let filter = scope_sql(scope, &mut args);
    args.push(Value::Integer(limit.clamp(1, 200) as i64));
    let sql = format!(
        "SELECT c.id, c.document_id, d.filename, c.section_id, s.title, c.chunk_index, c.page_start, c.page_end,
                c.loc_label, c.content, bm25(document_chunks_fts) AS score
         FROM document_chunks_fts
         JOIN document_chunks c ON c.id = document_chunks_fts.rowid
         JOIN documents d ON d.id = c.document_id
         LEFT JOIN document_sections s ON s.id = c.section_id
         WHERE document_chunks_fts MATCH ?1 AND d.course_id = ?2 AND d.status = 'completed'{filter}
         ORDER BY score LIMIT ?{}",
        args.len()
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params_from_iter(args), |r| {
        Ok(ChunkHit {
            chunk_id: r.get(0)?,
            document_id: r.get(1)?,
            filename: r.get(2)?,
            section_id: r.get(3)?,
            section_title: r.get(4)?,
            chunk_index: r.get(5)?,
            page_start: r.get(6)?,
            page_end: r.get(7)?,
            loc_label: r.get(8)?,
            content: r.get(9)?,
            score: r.get(10)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Search box: every term must appear (exact substring for Chinese).
pub fn search_course(conn: &Connection, course_id: i64, query: &str, document_id: Option<i64>, limit: usize) -> AppResult<SearchResult> {
    let scope = Scope { document_ids: document_id.into_iter().collect(), section_ids: vec![] };
    let terms = segment::highlight_terms(query, QueryMode::All);
    let hits = retrieve(conn, course_id, query, QueryMode::All, &scope, limit)?
        .into_iter()
        .map(|chunk| SearchHit {
            chunk_id: chunk.chunk_id,
            document_id: chunk.document_id,
            file_type: chunk.filename.rsplit('.').next().unwrap_or("").to_lowercase(),
            snippet: segment::snippet(&chunk.content, &terms, 160),
            filename: chunk.filename,
            section_id: chunk.section_id,
            section_title: chunk.section_title,
            page_start: chunk.page_start,
            page_end: chunk.page_end,
            loc_label: chunk.loc_label,
            score: chunk.score,
        })
        .collect();
    Ok(SearchResult { query: query.to_string(), terms, hits })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support;

    #[test]
    fn finds_two_character_chinese_words_and_ranks_by_relevance() {
        let (paths, conn) = test_support::course_with_document();
        let result = search_course(&conn, 1, "导数", None, 20).unwrap();
        assert!(result.hits.len() >= 2);
        assert!(result.hits.iter().all(|h| h.snippet.iter().any(|s| s.hit && s.text == "导数")));
        assert_eq!(result.hits[0].section_title.as_deref(), Some("第二章 导数"));
        assert_eq!(result.hits[0].loc_label, "第 3 页");
        // Exact search does not match characters that are merely nearby.
        assert!(search_course(&conn, 1, "导极", None, 20).unwrap().hits.is_empty());
        // Other courses are never searched.
        assert!(search_course(&conn, 2, "导数", None, 20).unwrap().hits.is_empty());
        // Question-style retrieval works without exact phrasing.
        let hits = retrieve(&conn, 1, "导数的几何意义是什么？", QueryMode::Any, &Scope::default(), 5).unwrap();
        assert_eq!(hits[0].section_title.as_deref(), Some("第二章 导数"));
        // Section scopes include sub-sections.
        let chapter_one: i64 = conn.query_row("SELECT id FROM document_sections WHERE title = '第一章 函数与极限'", [], |r| r.get(0)).unwrap();
        let scoped = retrieve(&conn, 1, "极限", QueryMode::Any, &Scope { document_ids: vec![], section_ids: vec![chapter_one] }, 5).unwrap();
        assert!(!scoped.is_empty() && scoped.iter().all(|h| h.page_start <= 2));
        drop(conn);
        test_support::cleanup(paths);
    }

    #[test]
    fn operator_like_input_is_safe() {
        let (paths, conn) = test_support::course_with_document();
        for query in ["\"", "AND OR NOT", "导数*", "NEAR(导 数)", "-", "^"] {
            assert!(search_course(&conn, 1, query, None, 10).is_ok(), "{query}");
        }
        drop(conn);
        test_support::cleanup(paths);
    }
}
