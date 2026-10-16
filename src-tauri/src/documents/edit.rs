//! Manual structure edits are transactional; page text remains the source of truth.
use super::{index, parse::{Extraction, ParsedBlock, ParsedPage}, Section};
use crate::error::{AppError, AppResult};
use rusqlite::{params, Connection};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum SectionEdit {
    Rename { title: String },
    Lock { locked: bool },
    Merge { next_id: i64 },
    Split { page: i64, block: usize, title: String },
}

fn title(value: &str) -> AppResult<&str> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 200 {
        return Err(AppError::invalid("章节名称须为 1–200 个字符"));
    }
    Ok(value)
}

fn parsed_pages(conn: &Connection, id: i64) -> AppResult<Vec<ParsedPage>> {
    let mut stmt = conn.prepare("SELECT page_number, label, width, height, has_ocr, loc_start, loc_end, blocks FROM document_pages WHERE document_id=?1 ORDER BY page_number")?;
    let rows = stmt.query_map([id], |r| Ok((ParsedPage {
        number: r.get(0)?, label: r.get(1)?, width: r.get(2)?, height: r.get(3)?,
        ocr: r.get(4)?, loc_start: r.get(5)?, loc_end: r.get(6)?, blocks: vec![],
    }, r.get::<_, String>(7)?)))?;
    rows.map(|row| { let (mut page, json) = row?; page.blocks = serde_json::from_str(&json)?; Ok(page) }).collect()
}

/// A lock protects a coordinate-based tree. Refuse ambiguous anchors before replacing files.
/// Retain its surrounding tree too, since changing siblings/parents changes locked ranges.
pub(super) fn validate_locked(conn: &Connection, id: i64, extraction: &Extraction) -> AppResult<()> {
    let locked: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM document_sections WHERE document_id=?1 AND locked=1)", [id], |r| r.get(0))?;
    if !locked { return Ok(()); }
    let old = parsed_pages(conn, id)?;
    let raw = |b: &ParsedBlock| b.raw_text.clone().unwrap_or_else(|| b.text.clone());
    let same_layout = old.len() == extraction.pages.len() && old.iter().zip(&extraction.pages).all(|(a, b)| {
        a.number == b.number && a.blocks.len() == b.blocks.len()
    });
    let (sections, _) = index::stored_sections(conn, id)?;
    let same_anchors = sections.iter().all(|s| [(s.page_start, s.block_start), (s.page_end, s.block_end)].iter().all(|(p, b)| {
        let before = old.iter().find(|u| u.number == *p).and_then(|u| u.blocks.get(*b));
        let after = extraction.pages.iter().find(|u| u.number == *p).and_then(|u| u.blocks.get(*b));
        match (before, after) { (Some(a), Some(b)) => raw(a) == raw(b), (None, None) => true, _ => false }
    }));
    if !same_layout || !same_anchors {
        return Err(AppError::invalid("原文块位置已变化，无法可靠保持锁定章节；已保留原结果。请先解锁章节再重新解析。"));
    }
    Ok(())
}

pub fn apply(conn: &mut Connection, document_id: i64, section_id: i64, edit: SectionEdit) -> AppResult<Vec<Section>> {
    let tx = conn.transaction()?;
    let document = super::get(&tx, document_id)?;
    if document.status != "completed" { return Err(AppError::invalid("请等待资料解析完成再调整章节")); }
    let sections = super::sections(&tx, document_id)?;
    let section = sections.iter().find(|s| s.id == section_id).ok_or(AppError::NotFound("章节"))?;
    if section.locked && !matches!(edit, SectionEdit::Lock { .. }) {
        return Err(AppError::invalid("请先解锁此章节再修改"));
    }
    let pages = parsed_pages(&tx, document_id)?;
    let mut reindex = false;
    match edit {
        SectionEdit::Rename { title: value } => {
            tx.execute("UPDATE document_sections SET title=?1, user_edited=1 WHERE id=?2", params![title(&value)?, section_id])?;
        }
        SectionEdit::Lock { locked } => {
            tx.execute("UPDATE document_sections SET locked=?1 WHERE id=?2", params![locked, section_id])?;
        }
        SectionEdit::Merge { next_id } => {
            let next = sections.iter().find(|s| s.ordinal > section.ordinal && s.parent_id == section.parent_id)
                .filter(|s| s.id == next_id && s.level == section.level)
                .ok_or_else(|| AppError::invalid("只能合并紧邻的下一个同级章节"))?;
            if next.locked { return Err(AppError::invalid("请先解锁下一个章节再合并")); }
            tx.execute("UPDATE document_sections SET page_end=?1, block_end=?2, user_edited=1 WHERE id=?3",
                params![next.page_end, next.block_end, section_id])?;
            tx.execute("UPDATE document_sections SET parent_id=?1 WHERE parent_id=?2", params![section_id, next_id])?;
            for (table, column) in index::SECTION_REFERENCES {
                tx.execute(&format!("UPDATE {table} SET {column}=?1 WHERE {column}=?2"), params![section_id, next_id])?;
            }
            index::remap_conversation_scopes(&tx, &HashMap::from([(next_id, section_id)]))?;
            tx.execute("DELETE FROM document_sections WHERE id=?1", [next_id])?;
            reindex = true;
        }
        SectionEdit::Split { page, block, title: value } => {
            let split = (page, block as i64);
            if split <= (section.page_start, section.block_start) || split > (section.page_end, section.block_end)
                || !pages.iter().any(|p| p.number == page && p.blocks.get(block).is_some_and(|b| b.kind != "synthetic" && !b.text.trim().is_empty())) {
                return Err(AppError::invalid("拆分位置必须是本节首块之后的正文块"));
            }
            let children: Vec<_> = sections.iter().filter(|s| s.parent_id == Some(section_id)).collect();
            if children.iter().any(|s| (s.page_start, s.block_start) < split && split <= (s.page_end, s.block_end)) {
                return Err(AppError::invalid("此位置位于子章节内部，请先选中该子章节拆分"));
            }
            let previous = if block > 0 { (page, block as i64 - 1) } else {
                let p = pages.iter().take_while(|p| p.number < page).last()
                    .ok_or_else(|| AppError::invalid("拆分位置无前置正文"))?;
                (p.number, p.blocks.len().saturating_sub(1) as i64)
            };
            tx.execute("UPDATE document_sections SET ordinal=ordinal+1 WHERE document_id=?1 AND ordinal>?2", params![document_id, section.ordinal])?;
            tx.execute("INSERT INTO document_sections (document_id,parent_id,ordinal,level,title,page_start,page_end,block_start,block_end,source,user_edited) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,'manual',1)",
                params![document_id, section.parent_id, section.ordinal+1, section.level, title(&value)?, page, section.page_end, block as i64, section.block_end])?;
            let new_id = tx.last_insert_rowid();
            tx.execute("UPDATE document_sections SET page_end=?1,block_end=?2,user_edited=1 WHERE id=?3", params![previous.0, previous.1, section_id])?;
            for child in children {
                if (child.page_start, child.block_start) >= split {
                    tx.execute("UPDATE document_sections SET parent_id=?1 WHERE id=?2", params![new_id, child.id])?;
                }
            }
            // Existing whole-section references remain on the first half. Descendant ids survive.
            reindex = true;
        }
    }
    // Keep preorder even when a split moves a subtree into the second half.
    let mut stmt = tx.prepare("SELECT id FROM document_sections WHERE document_id=?1 ORDER BY page_start,block_start,level,ordinal")?;
    let ids = stmt.query_map([document_id], |r| r.get::<_, i64>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
    drop(stmt);
    for (ordinal, id) in ids.iter().enumerate() {
        tx.execute("UPDATE document_sections SET ordinal=?1 WHERE id=?2", params![ordinal as i64, id])?;
    }
    if reindex {
        let (ranges, ids) = index::stored_sections(&tx, document_id)?;
        let structure = index::build_ranges(&pages, &document.loc_kind, &super::stem(&document.filename), ranges);
        let locs = pages.iter().map(|p| (p.number, (p.loc_start,p.loc_end))).collect();
        tx.execute("DELETE FROM document_chunks WHERE document_id=?1", [document_id])?;
        index::write_chunks(&tx, document_id, &document.loc_kind, &locs, &structure, &ids)?;
    }
    tx.commit()?;
    super::sections(conn, document_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{self, sample_extraction};

    #[test]
    fn merge_preserves_children_notes_quizzes_and_json_scopes() {
        let (paths, mut conn) = test_support::course_with_document();
        let sections = super::super::sections(&conn, 1).unwrap();
        let (first, child, second) = (sections[0].id, sections[1].id, sections[2].id);
        conn.execute("INSERT INTO notes(course_id,title,content,section_id) VALUES(1,'笔记','原内容',?1)", [second]).unwrap();
        conn.execute("INSERT INTO quizzes(course_id,title,section_id) VALUES(1,'练习',?1)", [second]).unwrap();
        conn.execute("INSERT INTO conversations(course_id,title,scope,section_id) VALUES(1,'对话',?1,?2)", params![format!("{{\"section_ids\": [{first}, {second}]}}"), second]).unwrap();
        let merged = apply(&mut conn, 1, first, SectionEdit::Merge { next_id: second }).unwrap();
        assert_eq!(merged.len(), 2);
        assert_eq!((merged[0].page_end, merged[1].id, merged[1].parent_id), (3, child, Some(first)));
        for table in ["notes", "quizzes", "conversations"] {
            assert_eq!(conn.query_row(&format!("SELECT section_id FROM {table}"), [], |r| r.get::<_, i64>(0)).unwrap(), first);
        }
        let scope: String = conn.query_row("SELECT scope FROM conversations", [], |r| r.get(0)).unwrap();
        assert_eq!(serde_json::from_str::<serde_json::Value>(&scope).unwrap()["section_ids"], serde_json::json!([first]));
        let text: String = conn.query_row("SELECT content FROM document_chunks WHERE section_id=?1 AND page_start=3", [first], |r| r.get(0)).unwrap();
        assert!(text.contains("切线斜率"));
        assert!(conn.query_row("PRAGMA foreign_key_check", [], |_| Ok(())).is_err());
        drop(conn);
        test_support::cleanup(paths);
    }

    #[test]
    fn split_same_page_reindexes_and_persists_without_changing_source() {
        let (paths, mut conn) = test_support::course_with_document();
        let first = super::super::sections(&conn, 1).unwrap()[0].id;
        let original = parsed_pages(&conn, 1).unwrap()[0].blocks[1].text.clone();
        let result = apply(&mut conn, 1, first, SectionEdit::Split { page: 1, block: 1, title: "定义与极限".into() }).unwrap();
        assert_eq!(result[0].block_end, 0);
        assert_eq!((result[1].page_start, result[1].block_start, result[1].page_end), (1, 1, 2));
        assert_eq!(result[2].parent_id, Some(result[1].id));
        let chunk: String = conn.query_row("SELECT content FROM document_chunks WHERE section_id=?1 ORDER BY chunk_index LIMIT 1", [result[1].id], |r| r.get(0)).unwrap();
        assert_eq!(chunk, original);
        assert_eq!(parsed_pages(&conn, 1).unwrap()[0].blocks[1].text, original);
        drop(conn);
        let conn = paths.connect().unwrap();
        assert!(super::super::sections(&conn, 1).unwrap()[1].user_edited);
        assert_eq!(conn.query_row("SELECT COUNT(*) FROM document_chunks", [], |r| r.get::<_, i64>(0)).unwrap(),
            conn.query_row("SELECT COUNT(*) FROM document_chunks_fts", [], |r| r.get::<_, i64>(0)).unwrap());
        drop(conn);
        test_support::cleanup(paths);
    }

    #[test]
    fn locked_manual_tree_survives_reparse_and_rejects_changed_anchors_atomically() {
        let (paths, mut conn) = test_support::course_with_document();
        let first = super::super::sections(&conn, 1).unwrap()[0].id;
        apply(&mut conn, 1, first, SectionEdit::Rename { title: "手动名称".into() }).unwrap();
        apply(&mut conn, 1, first, SectionEdit::Lock { locked: true }).unwrap();
        assert!(apply(&mut conn, 1, first, SectionEdit::Rename { title: "覆盖".into() }).is_err());
        super::super::apply_extraction(&mut conn, 1, &sample_extraction()).unwrap();
        let section = &super::super::sections(&conn, 1).unwrap()[0];
        assert_eq!((section.id, section.title.as_str(), section.locked), (first, "手动名称", true));
        let mut changed = sample_extraction();
        changed.pages[0].blocks[0].text = "位置已变化".into();
        assert!(super::super::apply_extraction(&mut conn, 1, &changed).is_err());
        assert_eq!(parsed_pages(&conn, 1).unwrap()[0].blocks[0].text, "第一章 函数与极限");
        apply(&mut conn, 1, first, SectionEdit::Lock { locked: false }).unwrap();
        super::super::apply_extraction(&mut conn, 1, &changed).unwrap();
        drop(conn);
        test_support::cleanup(paths);
    }

    #[test]
    fn invalid_splits_merges_and_cross_document_ids_do_not_mutate() {
        let (paths, mut conn) = test_support::course_with_document();
        let sections = super::super::sections(&conn, 1).unwrap();
        let first = sections[0].id;
        for edit in [SectionEdit::Split { page: 1, block: 0, title: "无效".into() },
            SectionEdit::Split { page: 2, block: 1, title: "切穿子节".into() },
            SectionEdit::Merge { next_id: sections[1].id }, SectionEdit::Rename { title: " ".into() }] {
            assert!(apply(&mut conn, 1, first, edit).is_err());
        }
        assert!(apply(&mut conn, 999, first, SectionEdit::Lock { locked: true }).is_err());
        assert_eq!(super::super::sections(&conn, 1).unwrap().len(), sections.len());
        drop(conn);
        test_support::cleanup(paths);
    }

    #[test]
    fn split_keeps_intervening_blank_pages_in_the_first_half() {
        let (paths, mut conn) = test_support::course_with_document();
        let mut extraction = sample_extraction();
        extraction.pages[1].blocks.clear();
        extraction.pages[2].blocks[0].heading = None;
        super::super::apply_extraction(&mut conn, 1, &extraction).unwrap();
        let id = super::super::sections(&conn, 1).unwrap()[0].id;
        let sections = apply(&mut conn, 1, id, SectionEdit::Split { page: 3, block: 0, title: "后半节".into() }).unwrap();
        assert_eq!((sections[0].page_end, sections[0].block_end, sections[1].page_start), (2, 0, 3));
        drop(conn);
        test_support::cleanup(paths);
    }
}
