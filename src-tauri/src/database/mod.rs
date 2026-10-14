use rusqlite::{Connection, Result};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::{AppHandle, Manager};

pub mod migrations;

/// Locations of everything ExamOps stores on disk. All paths live under the
/// per-user application data directory (`%APPDATA%\com.examops.app` on Windows).
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
        Ok(AppPaths::new(dir))
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
    recover_interrupted(&conn).map_err(|e| e.to_string())
}

pub fn recover_interrupted(conn: &Connection) -> Result<()> {
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
