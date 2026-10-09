use rusqlite::{Connection, Result};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

pub mod schema;

pub fn get_db_path(app_handle: &AppHandle) -> PathBuf {
    app_handle
        .path()
        .app_data_dir()
        .expect("Failed to get app data directory")
        .join("examops.db")
}

pub fn init_database(app_handle: &AppHandle) -> Result<()> {
    let db_path = get_db_path(app_handle);

    // Ensure parent directory exists
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).expect("Failed to create app data directory");
    }

    let conn = Connection::open(&db_path)?;
    schema::create_tables(&conn)?;

    Ok(())
}

pub fn get_connection(app_handle: &AppHandle) -> Result<Connection> {
    let db_path = get_db_path(app_handle);
    Connection::open(db_path)
}
