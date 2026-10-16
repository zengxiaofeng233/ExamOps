//! Turns parsed units into the chapter tree, retrieval chunks and FTS5 rows.

use super::parse::{Extraction, ParsedBlock, ParsedPage};
use crate::search::segment;
use rusqlite::{params, Transaction};
use std::collections::HashMap;

/// Chunks aim for a few hundred characters: small enough for precise citations,
/// large enough to carry a complete explanation.
const MAX_CHUNK: usize = 1000;
const PAGE_FLUSH: usize = 350;

#[derive(Debug, Clone, PartialEq)]
pub struct SectionDraft {
    pub level: u8,
    pub title: String,
    pub parent: Option<usize>,
    pub page_start: i64,
    pub page_end: i64,
    pub char_count: usize,
    pub source: String,
    pub block_start: usize,
    pub block_end: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChunkDraft {
    pub section: Option<usize>,
    pub page_start: i64,
    pub page_end: i64,
    pub text: String,
    pub unreliable_formula: bool,
}

#[derive(Debug, Default)]
pub struct Structure {
    pub sections: Vec<SectionDraft>,
    pub chunks: Vec<ChunkDraft>,
}

pub fn loc_label(loc_kind: &str, start: i64, end: i64) -> String {
    let (prefix, unit) = match loc_kind {
        "slide" => ("第 ", " 张幻灯片"),
        "paragraph" => ("第 ", " 段"),
        "line" => ("第 ", " 行"),
        _ => ("第 ", " 页"),
    };
    if start == end {
        format!("{prefix}{start}{unit}")
    } else {
        format!("{prefix}{start}–{end}{unit}")
    }
}

/// Location label for a chunk covering units `page_start..=page_end`.
pub fn chunk_label(loc_kind: &str, pages: &HashMap<i64, (i64, i64)>, page_start: i64, page_end: i64) -> String {
    if loc_kind == "line" {
        let start = pages.get(&page_start).map(|p| p.0).unwrap_or(page_start);
        let end = pages.get(&page_end).map(|p| p.1).unwrap_or(page_end);
        loc_label(loc_kind, start, end)
    } else {
        loc_label(loc_kind, page_start, page_end)
    }
}

fn split_long(text: &str) -> Vec<String> {
    let mut pieces = Vec::new();
    let mut current = String::new();
    for c in text.chars() {
        current.push(c);
        let len = current.chars().count();
        let boundary = matches!(c, '。' | '！' | '？' | '；' | '.' | '!' | '?' | ';' | '\n');
        if (boundary && len >= MAX_CHUNK * 2 / 3) || len >= MAX_CHUNK {
            pieces.push(std::mem::take(&mut current));
        }
    }
    if !current.trim().is_empty() {
        pieces.push(current);
    }
    pieces
}

struct Builder<'a> {
    loc_kind: &'a str,
    title: &'a str,
    has_headings: bool,
    structure: Structure,
    stack: Vec<usize>,
    current: Option<ChunkDraft>,
}

impl Builder<'_> {
    fn flush(&mut self) {
        if let Some(chunk) = self.current.take() {
            if !chunk.text.trim().is_empty() {
                self.structure.chunks.push(chunk);
            }
        }
    }

    fn open_section(&mut self, level: u8, title: &str, page: i64, block: usize, source: &str) {
        self.flush();
        while let Some(&top) = self.stack.last() {
            if self.structure.sections[top].level >= level {
                self.stack.pop();
            } else {
                break;
            }
        }
        // Slide titles (level 2) and leading level-2 headings hang under a document root.
        if level > 1 && self.stack.is_empty() {
            self.root(page);
            if let Some(root) = self.structure.sections.last_mut() {
                root.title = self.title.to_string();
                root.source = "container".into();
            }
        }
        let parent = self.stack.last().copied();
        self.structure.sections.push(SectionDraft {
            level,
            title: title.trim().to_string(),
            parent,
            page_start: page,
            page_end: page,
            char_count: 0,
            source: source.to_string(),
            block_start: block,
            block_end: block,
        });
        self.stack.push(self.structure.sections.len() - 1);
    }

    fn root(&mut self, page: i64) {
        let title = if self.has_headings && self.loc_kind != "slide" { "开头部分" } else { self.title };
        self.structure.sections.push(SectionDraft {
            level: 1,
            title: title.to_string(),
            parent: None,
            page_start: page,
            page_end: page,
            char_count: 0,
            source: if self.has_headings { "derived".into() } else { "fallback".into() },
            block_start: 0,
            block_end: 0,
        });
        self.stack.push(self.structure.sections.len() - 1);
    }

    fn add_text(&mut self, block: &ParsedBlock, page: i64) {
        let text = &block.text;
        if text.trim().is_empty() {
            return;
        }
        if self.stack.is_empty() {
            self.root(page);
        }
        let section = self.stack.last().copied();
        let atomic = block.formula || !block.issues.is_empty();
        let unreliable_formula = atomic && (block.recognition != "native" || !block.issues.is_empty());
        if atomic { self.flush(); }
        let pieces = if !atomic && text.chars().count() > MAX_CHUNK { split_long(text) } else { vec![text.to_string()] };
        for piece in pieces {
            let piece_len = piece.chars().count();
            let fits = self
                .current
                .as_ref()
                .map(|c| c.section == section && c.text.chars().count() + piece_len <= MAX_CHUNK)
                .unwrap_or(false);
            match self.current.as_mut() {
                Some(chunk) if fits => {
                    chunk.text.push('\n');
                    chunk.text.push_str(&piece);
                    chunk.page_end = page;
                }
                _ => {
                    self.flush();
                    self.current = Some(ChunkDraft { section, page_start: page, page_end: page, text: piece, unreliable_formula });
                }
            }
        }
        if atomic { self.flush(); }
    }

    fn new_page(&mut self) {
        if self.current.as_ref().map(|c| c.text.chars().count() >= PAGE_FLUSH).unwrap_or(false) {
            self.flush();
        }
    }
}

/// Builds sections and chunks. `title` (the file name) names the fallback root.
pub fn build(pages: &[ParsedPage], loc_kind: &str, title: &str) -> Structure {
    let has_headings = pages.iter().any(|p| p.blocks.iter().any(|b| b.heading.is_some()));

    let mut builder = Builder {
        loc_kind,
        title,
        has_headings,
        structure: Structure::default(),
        stack: Vec::new(),
        current: None,
    };
    for page in pages {
        builder.new_page();
        if page.blocks.is_empty() {
            for &index in &builder.stack {
                builder.structure.sections[index].page_end = page.number;
                builder.structure.sections[index].block_end = 0;
            }
        }
        for (block_index, block) in page.blocks.iter().enumerate() {
            match block.heading {
                Some(level) if !block.text.trim().is_empty() => {
                    let source = block.source.as_deref().unwrap_or("detected");
                    builder.open_section(level.clamp(1, 6), &block.text, page.number, block_index, source);
                    if block.kind != "synthetic" {
                        builder.add_text(block, page.number);
                    }
                }
                _ => builder.add_text(block, page.number),
            }
            for &index in &builder.stack {
                builder.structure.sections[index].page_end = page.number;
                builder.structure.sections[index].block_end = block_index;
            }
        }
    }
    builder.flush();
    let mut structure = builder.structure;

    // Section page ranges and sizes cover their whole subtree.
    for chunk in &structure.chunks {
        let mut cursor = chunk.section;
        while let Some(index) = cursor {
            let section = &mut structure.sections[index];
            section.page_end = section.page_end.max(chunk.page_end);
            section.page_start = section.page_start.min(chunk.page_start);
            section.char_count += chunk.text.chars().count();
            cursor = section.parent;
        }
    }
    let ends: Vec<(Option<usize>, i64)> = structure.sections.iter().map(|s| (s.parent, s.page_end)).collect();
    for (parent, end) in ends {
        let mut cursor = parent;
        while let Some(parent) = cursor {
            structure.sections[parent].page_end = structure.sections[parent].page_end.max(end);
            cursor = structure.sections[parent].parent;
        }
    }
    structure
}

/// Tables that point at `document_sections`; their references survive re-parsing.
pub(super) fn stored_sections(conn: &rusqlite::Connection, document_id: i64) -> rusqlite::Result<(Vec<SectionDraft>, Vec<i64>)> {
    let mut stmt = conn.prepare("SELECT id, parent_id, level, title, page_start, page_end, source, block_start, block_end FROM document_sections WHERE document_id=?1 ORDER BY ordinal")?;
    let mut sections = Vec::new();
    let mut ids = Vec::new();
    let rows = stmt.query_map([document_id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<i64>>(1)?, SectionDraft {
        level: r.get(2)?, title: r.get(3)?, parent: None, page_start: r.get(4)?, page_end: r.get(5)?,
        source: r.get(6)?, block_start: r.get(7)?, block_end: r.get(8)?, char_count: 0,
    })))?;
    for row in rows {
        let (id, parent, mut section) = row?;
        section.parent = parent.and_then(|p| ids.iter().position(|i| *i == p));
        ids.push(id);
        sections.push(section);
    }
    Ok((sections, ids))
}

/// Rebuild retrieval against explicit user boundaries without rediscovering headings.
pub(super) fn build_ranges(pages: &[ParsedPage], loc_kind: &str, title: &str, sections: Vec<SectionDraft>) -> Structure {
    let mut builder = Builder { loc_kind, title, has_headings: false,
        structure: Structure { sections, chunks: vec![] }, stack: vec![], current: None };
    for page in pages {
        builder.new_page();
        for (b, block) in page.blocks.iter().enumerate() {
            let owner = builder.structure.sections.iter().enumerate()
                .filter(|(_, s)| (s.page_start, s.block_start) <= (page.number, b) && (page.number, b) <= (s.page_end, s.block_end))
                .max_by_key(|(_, s)| s.level).map(|(i, _)| i);
            if builder.stack.last().copied() != owner { builder.flush(); }
            builder.stack = owner.into_iter().collect();
            if block.kind != "synthetic" { builder.add_text(block, page.number); }
        }
    }
    builder.flush();
    for chunk in &builder.structure.chunks {
        let mut cursor = chunk.section;
        while let Some(i) = cursor {
            builder.structure.sections[i].char_count += chunk.text.chars().count();
            cursor = builder.structure.sections[i].parent;
        }
    }
    builder.structure
}

/// Tables that point at `document_sections`; their references survive re-parsing.
pub const SECTION_REFERENCES: &[(&str, &str)] = &[
    ("knowledge_points", "section_id"),
    ("resources", "section_id"),
    ("path_steps", "section_id"),
    ("conversations", "section_id"),
    ("notes", "section_id"),
    ("study_state", "current_section_id"),
    ("quizzes", "section_id"),
    ("study_events", "section_id"),
];

/// Conversation scopes store section ids inside JSON; keep them pointing at
/// the re-created sections.
pub(super) fn remap_conversation_scopes(tx: &Transaction, remapped: &HashMap<i64, i64>) -> rusqlite::Result<()> {
    if remapped.is_empty() {
        return Ok(());
    }
    let scoped: Vec<(i64, String)> = {
        let mut stmt = tx.prepare("SELECT id, scope FROM conversations WHERE scope IS NOT NULL")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    for (id, scope) in scoped {
        let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&scope) else { continue };
        let Some(sections) = value.get_mut("section_ids").and_then(|v| v.as_array_mut()) else { continue };
        let mut changed = false;
        for section in sections.iter_mut() {
            if let Some(new_id) = section.as_i64().and_then(|old| remapped.get(&old)) {
                *section = serde_json::json!(new_id);
                changed = true;
            }
        }
        if changed {
            let mut seen = std::collections::HashSet::new();
            sections.retain(|v| seen.insert(v.to_string()));
            tx.execute("UPDATE conversations SET scope = ?1 WHERE id = ?2", params![value.to_string(), id])?;
        }
    }
    Ok(())
}

fn section_key(level: i64, title: &str) -> String {
    format!("{level}:{}", title.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase())
}

/// Replaces the stored structure of a document inside an open transaction.
pub fn store(tx: &Transaction, document_id: i64, title: &str, extraction: &Extraction) -> rusqlite::Result<Structure> {
    let locked: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM document_sections WHERE document_id=?1 AND locked=1)", [document_id], |r| r.get(0))?;
    let (structure, retained_ids) = if locked {
        let (sections, ids) = stored_sections(tx, document_id)?;
        (build_ranges(&extraction.pages, &extraction.loc_kind, title, sections), ids)
    } else {
        (build(&extraction.pages, &extraction.loc_kind, title), vec![])
    };

    // Remember old sections so references (knowledge points, path steps, notes...) can be remapped.
    let old: Vec<(i64, i64, String, i64, i64)> = {
        let mut stmt = tx.prepare(
            "SELECT id, level, title, page_start, block_start FROM document_sections WHERE document_id = ?1 ORDER BY ordinal",
        )?;
        let rows = stmt.query_map([document_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    tx.execute("DELETE FROM document_chunks WHERE document_id = ?1", [document_id])?;
    tx.execute("DELETE FROM document_pages WHERE document_id = ?1", [document_id])?;
    tx.execute("DELETE FROM document_images WHERE document_id = ?1", [document_id])?;

    let mut ids: Vec<i64> = retained_ids;
    if !locked {
        let mut insert = tx.prepare(
            "INSERT INTO document_sections (document_id, parent_id, ordinal, level, title, page_start, page_end, char_count, source, block_start, block_end)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        )?;
        for (ordinal, section) in structure.sections.iter().enumerate() {
            insert.execute(params![
                document_id,
                section.parent.map(|p| ids[p]),
                ordinal as i64,
                section.level,
                section.title,
                section.page_start,
                section.page_end,
                section.char_count as i64,
                section.source,
                section.block_start as i64,
                section.block_end as i64
            ])?;
            ids.push(tx.last_insert_rowid());
        }
    }

    if !locked && !old.is_empty() {
        let mut by_key: HashMap<String, Vec<i64>> = HashMap::new();
        for (index, section) in structure.sections.iter().enumerate() {
            by_key.entry(section_key(section.level as i64, &section.title)).or_default().push(ids[index]);
        }
        let mut used: HashMap<String, usize> = HashMap::new();
        let mut remapped: HashMap<i64, i64> = HashMap::new();
        for (old_id, level, title, page_start, block_start) in &old {
            let key = section_key(*level, title);
            let occurrence = used.entry(key.clone()).or_insert(0);
            let target = by_key.get(&key).and_then(|v| v.get(*occurrence)).copied().or_else(|| {
                // Same-page headings need a block anchor as well as the page.
                structure
                    .sections
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| (s.page_start, s.block_start as i64) <= (*page_start, *block_start)
                        && (*page_start, *block_start) <= (s.page_end, s.block_end as i64))
                    .max_by_key(|(_, s)| s.level)
                    .map(|(i, _)| ids[i])
            });
            *occurrence += 1;
            if let Some(new_id) = target {
                for (table, column) in SECTION_REFERENCES {
                    tx.execute(&format!("UPDATE {table} SET {column} = ?1 WHERE {column} = ?2"), params![new_id, old_id])?;
                }
                remapped.insert(*old_id, new_id);
            }
        }
        remap_conversation_scopes(tx, &remapped)?;
        let old_ids: Vec<String> = old.iter().map(|o| o.0.to_string()).collect();
        tx.execute(
            &format!("DELETE FROM document_sections WHERE document_id = ?1 AND id IN ({})", old_ids.join(",")),
            [document_id],
        )?;
    }

    let mut locs: HashMap<i64, (i64, i64)> = HashMap::new();
    {
        let mut insert = tx.prepare(
            "INSERT INTO document_pages (document_id, page_number, label, loc_start, loc_end, content, blocks, width, height, has_ocr)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        )?;
        for page in &extraction.pages {
            let content = page.blocks.iter().map(|b| b.text.as_str()).collect::<Vec<_>>().join("\n");
            let blocks = serde_json::to_string(&page.blocks).unwrap_or_else(|_| "[]".into());
            let label = if page.label.is_empty() { loc_label(&extraction.loc_kind, page.loc_start, page.loc_end) } else { page.label.clone() };
            insert.execute(params![
                document_id, page.number, label, page.loc_start, page.loc_end, content, blocks, page.width, page.height, page.ocr
            ])?;
            locs.insert(page.number, (page.loc_start, page.loc_end));
        }
    }
    write_chunks(tx, document_id, &extraction.loc_kind, &locs, &structure, &ids)?;
    {
        let mut insert = tx.prepare(
            "INSERT INTO document_images (document_id, page_number, ordinal, file_path, width, height, ocr_text)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )?;
        for image in &extraction.images {
            insert.execute(params![document_id, image.page, image.ordinal, image.path, image.width, image.height, image.ocr_text])?;
        }
    }
    let char_count: usize = structure.chunks.iter().map(|c| c.text.chars().count()).sum();
    tx.execute(
        "UPDATE documents SET page_count = ?1, loc_kind = ?2, char_count = ?3, structure_source = ?4,
         warnings = ?5, indexed_at = datetime('now', 'localtime') WHERE id = ?6",
        params![
            extraction.pages.len() as i64,
            extraction.loc_kind,
            char_count as i64,
            extraction.structure_source,
            serde_json::to_string(&extraction.warnings).unwrap_or_else(|_| "[]".into()),
            document_id
        ],
    )?;
    Ok(structure)
}

pub(super) fn write_chunks(tx: &Transaction, document_id: i64, loc_kind: &str,
    locs: &HashMap<i64, (i64, i64)>, structure: &Structure, ids: &[i64]) -> rusqlite::Result<()> {
    let mut insert = tx.prepare(
        "INSERT INTO document_chunks (document_id, section_id, chunk_index, page_start, page_end, loc_label, content, char_count, unreliable_formula)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
    )?;
    let mut fts = tx.prepare("INSERT INTO document_chunks_fts (rowid, body) VALUES (?1, ?2)")?;
    for (index, chunk) in structure.chunks.iter().enumerate() {
        insert.execute(params![
            document_id,
            chunk.section.map(|s| ids[s]),
            index as i64,
            chunk.page_start,
            chunk.page_end,
            chunk_label(loc_kind, locs, chunk.page_start, chunk.page_end),
            chunk.text,
            chunk.text.chars().count() as i64,
            chunk.unreliable_formula
        ])?;
        fts.execute(params![tx.last_insert_rowid(), segment::index_text(&chunk.text)])?;
    }
    for (section, id) in structure.sections.iter().zip(ids) {
        tx.execute("UPDATE document_sections SET char_count=?1 WHERE id=?2", params![section.char_count as i64, id])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(text: &str, heading: Option<u8>) -> ParsedBlock {
        ParsedBlock { text: text.into(), heading, kind: "text".into(), ..Default::default() }
    }

    fn page(number: i64, blocks: Vec<ParsedBlock>) -> ParsedPage {
        ParsedPage { number, label: String::new(), width: None, height: None, ocr: false, loc_start: number, loc_end: number, blocks }
    }

    #[test]
    fn headings_build_a_tree_and_chunks_keep_pages() {
        let pages = vec![
            page(1, vec![block("封面 课程讲义", None)]),
            page(2, vec![block("第一章 函数", Some(1)), block("函数的定义。", None), block("1.1 映射", Some(2)), block("映射的定义。", None)]),
            page(3, vec![block("映射的例子。", None)]),
            page(4, vec![block("第二章 极限", Some(1)), block("极限的定义。", None)]),
        ];
        let s = build(&pages, "page", "讲义");
        let titles: Vec<_> = s.sections.iter().map(|x| (x.level, x.title.as_str(), x.parent)).collect();
        assert_eq!(titles, vec![(1, "开头部分", None), (1, "第一章 函数", None), (2, "1.1 映射", Some(1)), (1, "第二章 极限", None)]);
        assert_eq!((s.sections[1].page_start, s.sections[1].page_end), (2, 3));
        assert_eq!((s.sections[2].page_start, s.sections[2].page_end), (2, 3));
        let mapping = s.chunks.iter().find(|c| c.text.contains("映射的例子")).unwrap();
        assert_eq!(mapping.section, Some(2));
        assert_eq!((mapping.page_start, mapping.page_end), (2, 3));
        assert!(s.chunks.iter().all(|c| !c.text.is_empty()));
    }

    #[test]
    fn no_headings_do_not_invent_ten_page_chapters() {
        let pages: Vec<_> = (1..=35).map(|n| page(n, vec![block(&format!("第{n}页正文"), None)])).collect();
        let s = build(&pages, "page", "扫描教材");
        assert_eq!(s.sections[0].title, "扫描教材");
        assert_eq!(s.sections.len(), 1);
        assert_eq!((s.sections[0].page_start, s.sections[0].page_end), (1, 35));
        let short = build(&[page(1, vec![block("正文", None)])], "page", "短文");
        assert_eq!(short.sections.len(), 1);
        assert_eq!(short.chunks[0].section, Some(0));
    }

    #[test]
    fn missing_parent_uses_root_without_promoting_real_levels() {
        let slides = vec![page(1, vec![block("随机事件", Some(2)), block("定义", None)]), page(2, vec![block("概率", Some(2))])];
        let s = build(&slides, "slide", "第一讲");
        assert_eq!(s.sections[0].title, "第一讲");
        assert_eq!(s.sections[1].parent, Some(0));
        let doc = vec![page(1, vec![block("背景", Some(2)), block("内容", None)])];
        let d = build(&doc, "paragraph", "笔记");
        assert_eq!((d.sections[1].level, d.sections[1].title.as_str()), (2, "背景"));
    }

    #[test]
    fn long_text_is_split_and_small_blocks_merge() {
        let long = "这是一句话。".repeat(400);
        let s = build(&[page(1, vec![block(&long, None)])], "page", "长文");
        assert!(s.chunks.len() >= 3);
        assert!(s.chunks.iter().all(|c| c.text.chars().count() <= MAX_CHUNK));
        let small: Vec<_> = (0..5).map(|i| block(&format!("短句{i}"), None)).collect();
        assert_eq!(build(&[page(1, small)], "page", "短").chunks.len(), 1);
    }

    #[test]
    fn same_page_sections_have_disjoint_block_boundaries() {
        let s = build(&[page(1, vec![block("第一章", Some(1)), block("甲", None),
            block("第二章", Some(1)), block("乙", None)])], "page", "教材");
        assert_eq!((s.sections[0].block_start, s.sections[0].block_end), (0, 1));
        assert_eq!((s.sections[1].block_start, s.sections[1].block_end), (2, 3));
        assert_eq!(s.chunks[0].text, "第一章\n甲");
        assert_eq!(s.chunks[1].text, "第二章\n乙");
    }

    #[test]
    fn blank_pages_before_next_heading_belong_to_the_previous_section() {
        let s = build(&[page(1, vec![block("第一章", Some(1))]), page(2, vec![]),
            page(3, vec![block("第二章", Some(1))])], "page", "教材");
        assert_eq!(s.sections[0].page_end, 2);
        assert_eq!(s.sections[1].page_start, 3);
    }

    #[test]
    fn long_formula_keeps_all_symbols_and_carries_recognition_warning() {
        let formula = "𝑥² + ∫₀¹ f(x) dx ≠ α\n".repeat(100);
        let mut math = block(&formula, None);
        math.formula = true;
        math.recognition = "unverified".into();
        let s = build(&[page(1, vec![block("前文", None), math, block("后文", None)])], "page", "教材");
        assert_eq!(s.chunks.len(), 3);
        assert_eq!(s.chunks[1].text, formula);
        assert!(s.chunks[1].unreliable_formula);
        assert!(!s.chunks[0].unreliable_formula && !s.chunks[2].unreliable_formula);
    }

    #[test]
    fn labels_follow_location_kind() {
        assert_eq!(loc_label("page", 3, 3), "第 3 页");
        assert_eq!(loc_label("slide", 2, 4), "第 2–4 张幻灯片");
        let locs = HashMap::from([(1, (1, 5)), (2, (7, 9))]);
        assert_eq!(chunk_label("line", &locs, 1, 2), "第 1–9 行");
    }
}
