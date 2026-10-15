use rusqlite::{Connection, Result};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::{AppHandle, Manager};

pub mod migrations;

/// Locations of everything ExamOps stores on disk. All paths live under the
/// chosen data directory (defaults to `%APPDATA%\com.examops.app` on Windows).
#[derive(Debug, Clone)]
pub struct AppPaths {
    pub data_dir: PathBuf,
    pub db_path: PathBuf,
    /// Application-owned copies of imported materials: `documents/<course>/<doc>.<ext>`.
    pub documents_dir: PathBuf,
    /// Re-creatable files such as rendered page previews.
    pub cache_dir: PathBuf,
}

impl AppPaths {
    pub fn new(data_dir: PathBuf) -> Self {
        AppPaths {
            db_path: data_dir.join("examops.db"),
            documents_dir: data_dir.join("documents"),
            cache_dir: data_dir.join("cache"),
            data_dir,
        }
    }

    pub fn resolve(app_handle: &AppHandle) -> std::result::Result<Self, String> {
        let dir = app_handle
            .path()
            .app_data_dir()
            .map_err(|e| format!("无法定位应用数据目录：{e}"))?;
        let (storage, paths) = crate::storage::StorageState::start(dir).map_err(|e| e.to_string())?;
        app_handle.manage(storage);
        Ok(paths)
    }

    pub fn connect(&self) -> Result<Connection> {
        open_connection(&self.db_path)
    }
}

pub fn get_connection(app_handle: &AppHandle) -> Result<Connection> {
    let paths = app_handle.state::<AppPaths>();
    paths.connect()
}

/// Opens a connection with the pragmas every connection needs.
pub fn open_connection(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    conn.busy_timeout(Duration::from_secs(10))?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    Ok(conn)
}

/// Opens (creating if needed) and migrates a database file. Used by start-up and tests.
pub fn open_and_migrate(path: &Path) -> std::result::Result<Connection, String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("无法创建数据目录：{e}"))?;
    }
    let mut conn = open_connection(path).map_err(|e| e.to_string())?;
    // WAL lets the UI read while an import or AI response is being written.
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(|e| e.to_string())?;
    migrations::migrate(&mut conn)?;
    Ok(conn)
}

/// Start-up: migrate the schema and recover work interrupted by a crash or forced exit.
pub fn init_database(paths: &AppPaths) -> std::result::Result<(), String> {
    for dir in [&paths.data_dir, &paths.documents_dir, &paths.cache_dir] {
        std::fs::create_dir_all(dir).map_err(|e| format!("无法创建数据目录：{e}"))?;
    }
    let conn = open_and_migrate(&paths.db_path)?;
    recover_interrupted(&conn).map_err(|e| e.to_string())?;
    remove_abandoned_staging(&paths.documents_dir);
    Ok(())
}

/// Imports and re-parses cut off by a crash or forced exit. A re-parse of a
/// document that already had a good result keeps it (as a failed re-parse
/// does); a first import is marked failed so the student can parse it again.
pub fn recover_interrupted(conn: &Connection) -> Result<()> {
    conn.execute(
        "UPDATE documents SET status = 'completed'
        WHERE status IN ('pending', 'processing')
          AND EXISTS (SELECT 1 FROM document_contents c WHERE c.document_id = documents.id AND c.content <> '')",
        [],
    )?;
    conn.execute(
        "UPDATE document_contents SET error = '上次正文提取被中断，请重新解析'
        WHERE document_id IN (SELECT id FROM documents WHERE status IN ('pending', 'processing'))",
        [],
    )?;
    conn.execute(
        "UPDATE documents SET status = 'failed' WHERE status IN ('pending', 'processing')",
        [],
    )?;
    Ok(())
}

/// Figures extracted by a parse that never finished (`documents/<course>/<id>_figures.new`).
fn remove_abandoned_staging(documents_dir: &Path) {
    let Ok(courses) = std::fs::read_dir(documents_dir) else { return };
    for course in courses.flatten() {
        let Ok(entries) = std::fs::read_dir(course.path()) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && path.extension().is_some_and(|ext| ext == "new") {
                let _ = std::fs::remove_dir_all(path);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::test_support;
    use std::time::{Duration, Instant};

    #[test]
    fn work_cut_off_by_a_crash_is_recovered_at_start_up() {
        let (paths, conn) = test_support::course_with_document();
        // Document 1 was parsed before; a re-parse of it was running. Document 2 was a first import.
        conn.execute("UPDATE documents SET status = 'processing' WHERE id = 1", []).unwrap();
        conn.execute(
            "INSERT INTO documents (course_id, filename, file_type, file_path, file_size, status) VALUES (1, '新讲义.pdf', 'pdf', '', 1, 'processing')",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO document_contents (document_id) VALUES (2)", []).unwrap();
        let staging = crate::documents::figures_dir(&paths.documents_dir, 1, 1).with_extension("new");
        std::fs::create_dir_all(&staging).unwrap();
        drop(conn);

        super::init_database(&paths).unwrap();
        let conn = paths.connect().unwrap();
        let first = crate::documents::get(&conn, 1).unwrap();
        assert_eq!((first.status.as_str(), first.error), ("completed", None));
        assert!(crate::documents::body(&conn, 1).unwrap().contains("函数"));
        let second = crate::documents::get(&conn, 2).unwrap();
        assert_eq!(second.status, "failed");
        assert_eq!(second.error.as_deref(), Some("上次正文提取被中断，请重新解析"));
        assert!(!staging.exists());
        drop(conn);
        test_support::cleanup(paths);
    }

    #[test]
    fn concurrent_writers_wait_instead_of_failing() {
        let paths = test_support::temp_paths("busy");
        let first = paths.connect().unwrap();
        first.execute_batch("BEGIN IMMEDIATE; INSERT INTO courses (name) VALUES ('甲');").unwrap();
        let other = paths.clone();
        let writer = std::thread::spawn(move || {
            let second = other.connect().unwrap();
            let started = Instant::now();
            second.execute("INSERT INTO courses (name) VALUES ('乙')", []).unwrap();
            started.elapsed()
        });
        std::thread::sleep(Duration::from_millis(300));
        first.execute_batch("COMMIT").unwrap();
        // The second writer waited for the lock (busy timeout) instead of failing with "database is locked".
        assert!(writer.join().unwrap() >= Duration::from_millis(200));
        let count: i64 = first.query_row("SELECT COUNT(*) FROM courses", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 2);
        drop(first);
        test_support::cleanup(paths);
    }
}
