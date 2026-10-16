-- Block coordinates, original text and recognition issues are stored in the
-- existing document_pages.blocks JSON, preserving the original imported file.
-- Block boundaries are zero-based, inclusive, within each stored page.
ALTER TABLE document_sections ADD COLUMN block_start INTEGER NOT NULL DEFAULT 0;
ALTER TABLE document_sections ADD COLUMN block_end INTEGER NOT NULL DEFAULT 2147483647;
ALTER TABLE document_sections ADD COLUMN user_edited INTEGER NOT NULL DEFAULT 0;
ALTER TABLE document_sections ADD COLUMN locked INTEGER NOT NULL DEFAULT 0;
ALTER TABLE document_chunks ADD COLUMN unreliable_formula INTEGER NOT NULL DEFAULT 0;
