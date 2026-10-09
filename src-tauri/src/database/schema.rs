use rusqlite::{Connection, Result};

pub fn create_tables(conn: &Connection) -> Result<()> {
    // Courses table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS courses (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
        )",
        [],
    )?;

    // Documents table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS documents (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            course_id INTEGER NOT NULL,
            filename TEXT NOT NULL,
            file_type TEXT NOT NULL,
            file_path TEXT NOT NULL,
            file_size INTEGER NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            created_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
            FOREIGN KEY (course_id) REFERENCES courses(id) ON DELETE CASCADE
        )",
        [],
    )?;

    // Document chunks table (for RAG)
    conn.execute(
        "CREATE TABLE IF NOT EXISTS document_chunks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            document_id INTEGER NOT NULL,
            chunk_index INTEGER NOT NULL,
            content TEXT NOT NULL,
            page_number INTEGER,
            created_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
            FOREIGN KEY (document_id) REFERENCES documents(id) ON DELETE CASCADE
        )",
        [],
    )?;

    // Full-text search index
    conn.execute(
        "CREATE VIRTUAL TABLE IF NOT EXISTS document_chunks_fts
         USING fts5(content, document_id UNINDEXED, chunk_id UNINDEXED)",
        [],
    )?;

    // Study sessions table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS study_sessions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            course_id INTEGER NOT NULL,
            chapter_name TEXT,
            last_position TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
            FOREIGN KEY (course_id) REFERENCES courses(id) ON DELETE CASCADE
        )",
        [],
    )?;

    // Chat messages table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS chat_messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            course_id INTEGER NOT NULL,
            session_id INTEGER,
            role TEXT NOT NULL,
            content TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
            FOREIGN KEY (course_id) REFERENCES courses(id) ON DELETE CASCADE,
            FOREIGN KEY (session_id) REFERENCES study_sessions(id) ON DELETE SET NULL
        )",
        [],
    )?;

    // Notes table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS notes (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            course_id INTEGER NOT NULL,
            title TEXT NOT NULL,
            content TEXT NOT NULL,
            document_id INTEGER,
            page_reference TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
            FOREIGN KEY (course_id) REFERENCES courses(id) ON DELETE CASCADE,
            FOREIGN KEY (document_id) REFERENCES documents(id) ON DELETE SET NULL
        )",
        [],
    )?;

    Ok(())
}
