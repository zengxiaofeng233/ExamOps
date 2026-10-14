use crate::database::{self, migrations, AppPaths};
use crate::error::{AppError, AppResult};
use crate::settings::{self, AppSettings, LAST_LOCATION_KEY};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;

#[derive(Debug, Serialize)]
pub struct AppInfo {
    pub version: String,
    pub data_dir: String,
    pub database_bytes: u64,
    pub documents_bytes: u64,
    pub schema_version: u32,
    pub platform: String,
}

/// Where the user was last time; restored on the next launch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastLocation {
    pub path: String,
    pub course_id: Option<i64>,
    #[serde(default)]
    pub saved_at: String,
}

fn dir_size(path: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| match entry.metadata() {
            Ok(meta) if meta.is_dir() => dir_size(&entry.path()),
            Ok(meta) => meta.len(),
            Err(_) => 0,
        })
        .sum()
}

#[tauri::command]
pub fn get_app_settings(app_handle: AppHandle) -> AppResult<AppSettings> {
    let conn = database::get_connection(&app_handle)?;
    settings::load(&conn)
}

#[tauri::command]
pub fn update_app_settings(app_handle: AppHandle, settings: AppSettings) -> AppResult<AppSettings> {
    let conn = database::get_connection(&app_handle)?;
    settings::save(&conn, settings)
}

#[tauri::command]
pub fn get_app_info(app_handle: AppHandle, paths: State<'_, AppPaths>) -> AppResult<AppInfo> {
    let conn = paths.connect()?;
    let wal = paths.db_path.with_extension("db-wal");
    Ok(AppInfo {
        version: app_handle.package_info().version.to_string(),
        data_dir: paths.data_dir.to_string_lossy().into_owned(),
        database_bytes: std::fs::metadata(&paths.db_path).map(|m| m.len()).unwrap_or(0)
            + std::fs::metadata(wal).map(|m| m.len()).unwrap_or(0),
        documents_bytes: dir_size(&paths.documents_dir),
        schema_version: migrations::current_version(&conn)?,
        platform: format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
    })
}

#[tauri::command]
pub fn open_data_dir(app_handle: AppHandle) -> AppResult<()> {
    let paths = app_handle.state::<AppPaths>();
    app_handle
        .opener()
        .open_path(paths.data_dir.to_string_lossy(), None::<&str>)
        .map_err(|e| AppError::msg(format!("无法打开数据目录：{e}")))
}

#[tauri::command]
pub fn save_last_location(app_handle: AppHandle, location: LastLocation) -> AppResult<()> {
    if !location.path.starts_with('/') || location.path.len() > 500 {
        return Err(AppError::invalid("位置无效"));
    }
    let conn = database::get_connection(&app_handle)?;
    let stamped = LastLocation {
        saved_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        ..location
    };
    settings::set_value(&conn, LAST_LOCATION_KEY, &stamped)
}

#[tauri::command]
pub fn get_last_location(app_handle: AppHandle) -> AppResult<Option<LastLocation>> {
    let conn = database::get_connection(&app_handle)?;
    let location: Option<LastLocation> = settings::get_value(&conn, LAST_LOCATION_KEY)?;
    // Drop locations that point at a course which no longer exists.
    Ok(match location {
        Some(loc) => match loc.course_id {
            Some(course_id) => {
                let exists: bool = conn.query_row(
                    "SELECT EXISTS(SELECT 1 FROM courses WHERE id = ?1)",
                    [course_id],
                    |r| r.get(0),
                )?;
                exists.then_some(loc)
            }
            None => Some(loc),
        },
        None => None,
    })
}
