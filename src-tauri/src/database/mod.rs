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
    recover_figure_swaps(paths, &conn).map_err(|e| e.to_string())?;
    recover_interrupted(&conn).map_err(|e| e.to_string())?;
    remove_abandoned_staging(&paths.documents_dir, &conn);
    Ok(())
}

/// Finish or roll back a directory swap interrupted around the final DB commit.
fn recover_figure_swaps(paths: &AppPaths, conn: &Connection) -> std::io::Result<()> {
    let mut stmt = conn.prepare("SELECT d.id,d.course_id,COALESCE(j.figure_swap_committed,1) FROM documents d LEFT JOIN document_jobs j ON j.document_id=d.id")
        .map_err(std::io::Error::other)?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?,r.get::<_,bool>(2)?)))
        .map_err(std::io::Error::other)?;
    for row in rows {
        let (id,course,committed) = row.map_err(std::io::Error::other)?;
        let directory = crate::documents::figures_dir(&paths.documents_dir,course,id);
        let backup = directory.with_extension("old");
        if backup.is_dir() {
            if committed { std::fs::remove_dir_all(backup)?; }
            else {
                if directory.exists() { std::fs::remove_dir_all(&directory)?; }
                std::fs::rename(backup,directory)?;
            }
        }
    }
    Ok(())
}

/// Requeue interrupted jobs. Completed data stays available during a re-parse;
/// first imports replay validated checkpoints into provisional page storage.
pub fn recover_interrupted(conn: &Connection) -> Result<()> {
    conn.execute("INSERT OR IGNORE INTO document_jobs(document_id,keep_previous)
        SELECT d.id, EXISTS(SELECT 1 FROM document_contents c WHERE c.document_id=d.id AND c.content <> '')
        FROM documents d WHERE d.status IN ('pending','processing')", [])?;
    conn.execute("UPDATE document_jobs SET keep_previous=1 WHERE state IN ('running','pending') AND EXISTS(SELECT 1 FROM documents d WHERE d.id=document_id AND d.indexed_at IS NOT NULL)", [])?;
    conn.execute("UPDATE document_jobs SET state='pending' WHERE state='running'", [])?;
    conn.execute("UPDATE documents SET status='pending' WHERE id IN (SELECT document_id FROM document_jobs WHERE state='pending')", [])?;
    Ok(())
}

/// Figures extracted by a parse that never finished (`documents/<course>/<id>_figures.new`).
fn remove_abandoned_staging(documents_dir: &Path, conn: &Connection) {
    let Ok(courses) = std::fs::read_dir(documents_dir) else { return };
    for course in courses.flatten() {
        let Ok(entries) = std::fs::read_dir(course.path()) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && path.extension().is_some_and(|ext| ext == "new") {
                let id = path.file_stem().and_then(|s| s.to_str()).and_then(|s| s.strip_suffix("_figures")).and_then(|s| s.parse::<i64>().ok());
                let pending = id.is_some_and(|id| conn.query_row("SELECT EXISTS(SELECT 1 FROM document_jobs WHERE document_id=?1 AND state != 'done')", [id], |r| r.get::<_,bool>(0)).unwrap_or(true));
                if !pending { let _ = std::fs::remove_dir_all(path); }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::test_support;
    use std::time::{Duration, Instant};

    #[test]
    fn interrupted_figure_swap_restores_old_files_until_database_commit() {
        let (paths, mut conn) = test_support::course_with_document();
        crate::documents::jobs::enqueue(&mut conn,1,true).unwrap();
        crate::documents::jobs::begin(&mut conn,1).unwrap();
        conn.execute("UPDATE document_jobs SET figure_swap_committed=0 WHERE document_id=1",[]).unwrap();
        let final_dir = crate::documents::figures_dir(&paths.documents_dir,1,1);
        let backup = final_dir.with_extension("old");
        std::fs::create_dir_all(&final_dir).unwrap();
        std::fs::create_dir_all(&backup).unwrap();
        std::fs::write(final_dir.join("image.png"),b"uncommitted new image").unwrap();
        std::fs::write(backup.join("image.png"),b"previous good image").unwrap();
        drop(conn);
        super::init_database(&paths).unwrap();
        assert_eq!(std::fs::read(final_dir.join("image.png")).unwrap(),b"previous good image");
        assert!(!backup.exists());
        let conn = paths.connect().unwrap();
        // A new job may already be pending after the previous result committed.
        conn.execute("UPDATE document_jobs SET state='pending',figure_swap_committed=1 WHERE document_id=1",[]).unwrap();
        std::fs::create_dir_all(&backup).unwrap();
        std::fs::write(backup.join("image.png"),b"obsolete image").unwrap();
        super::recover_figure_swaps(&paths,&conn).unwrap();
        assert!(!backup.exists());
        assert_eq!(std::fs::read(final_dir.join("image.png")).unwrap(),b"previous good image");
        drop(conn); test_support::cleanup(paths);
    }

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
        assert_eq!((first.status.as_str(), first.error), ("pending", None));
        assert!(crate::documents::body(&conn, 1).unwrap().contains("函数"));
        let second = crate::documents::get(&conn, 2).unwrap();
        assert_eq!(second.status, "pending");
        assert_eq!(second.error, None);
        assert!(staging.exists());
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
