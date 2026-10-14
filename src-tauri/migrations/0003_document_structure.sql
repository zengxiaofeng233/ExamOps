-- 0003 document structure: pages/units, recognised chapters, retrieval chunks,
-- FTS5 index and extracted figures (Stage 2 · Document Engine).

ALTER TABLE documents ADD COLUMN page_count INTEGER NOT NULL DEFAULT 0;
-- page | slide | paragraph | line : what a "unit" is for this document
ALTER TABLE documents ADD COLUMN loc_kind TEXT NOT NULL DEFAULT 'page';
ALTER TABLE documents ADD COLUMN char_count INTEGER NOT NULL DEFAULT 0;
-- outline | styles | markdown | slides | patterns | layout | none | legacy
ALTER TABLE documents ADD COLUMN structure_source TEXT NOT NULL DEFAULT '';
ALTER TABLE documents ADD COLUMN warnings TEXT NOT NULL DEFAULT '[]';
ALTER TABLE documents ADD COLUMN indexed_at TEXT;

-- One row per unit: PDF page, PPT slide, Word paragraph or block of text lines.
CREATE TABLE document_pages (
    document_id INTEGER NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    page_number INTEGER NOT NULL,
    label TEXT NOT NULL DEFAULT '',
    loc_start INTEGER NOT NULL,
    loc_end INTEGER NOT NULL,
    content TEXT NOT NULL DEFAULT '',
    -- JSON array of {"text", "heading", "kind"} blocks for the reader
    blocks TEXT NOT NULL DEFAULT '[]',
    width REAL,
    height REAL,
    has_ocr INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (document_id, page_number)
);

-- Recognised chapter tree (level 1 = chapter).
CREATE TABLE document_sections (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    document_id INTEGER NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    parent_id INTEGER REFERENCES document_sections(id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL,
    level INTEGER NOT NULL,
    title TEXT NOT NULL,
    page_start INTEGER NOT NULL,
    page_end INTEGER NOT NULL,
    char_count INTEGER NOT NULL DEFAULT 0,
    source TEXT NOT NULL DEFAULT ''
);
CREATE INDEX idx_document_sections_document ON document_sections(document_id, ordinal);

-- The phase-one chunk tables were never populated; rebuild them with locations.
DROP TABLE IF EXISTS document_chunks_fts;
DROP TABLE IF EXISTS document_chunks;

CREATE TABLE document_chunks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    document_id INTEGER NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    section_id INTEGER REFERENCES document_sections(id) ON DELETE SET NULL,
    chunk_index INTEGER NOT NULL,
    page_start INTEGER NOT NULL,
    page_end INTEGER NOT NULL,
    loc_label TEXT NOT NULL,
    content TEXT NOT NULL,
    char_count INTEGER NOT NULL
);
CREATE INDEX idx_document_chunks_document ON document_chunks(document_id, chunk_index);
CREATE INDEX idx_document_chunks_section ON document_chunks(section_id);

-- rowid = document_chunks.id; body holds the CJK-segmented text (see search/segment.rs).
CREATE VIRTUAL TABLE document_chunks_fts USING fts5(body, tokenize = 'unicode61 remove_diacritics 2');

CREATE TRIGGER document_chunks_after_delete AFTER DELETE ON document_chunks BEGIN
    DELETE FROM document_chunks_fts WHERE rowid = old.id;
END;

CREATE TABLE document_images (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    document_id INTEGER NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    page_number INTEGER NOT NULL,
    ordinal INTEGER NOT NULL,
    file_path TEXT NOT NULL,
    width INTEGER NOT NULL,
    height INTEGER NOT NULL,
    ocr_text TEXT NOT NULL DEFAULT ''
);
CREATE INDEX idx_document_images_page ON document_images(document_id, page_number);
