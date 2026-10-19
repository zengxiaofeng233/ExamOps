-- Durable OCR queue and provisional pages. Existing completed data stays intact.
CREATE TABLE document_jobs (
    document_id INTEGER PRIMARY KEY REFERENCES documents(id) ON DELETE CASCADE,
    state TEXT NOT NULL DEFAULT 'pending',
    keep_previous INTEGER NOT NULL DEFAULT 0,
    completed_units INTEGER NOT NULL DEFAULT 0,
    total_units INTEGER NOT NULL DEFAULT 0,
    figure_swap_committed INTEGER NOT NULL DEFAULT 1,
    error TEXT
);
CREATE TABLE document_page_drafts (
    document_id INTEGER NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    page_number INTEGER NOT NULL,
    page_json TEXT NOT NULL,
    PRIMARY KEY (document_id, page_number)
);
