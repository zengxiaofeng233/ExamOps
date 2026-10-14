//! User preferences stored in the `app_settings` table as JSON values.
//!
//! Every field has a default, so settings saved by an older version keep
//! working when new fields are added.

use crate::error::AppResult;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

const APP_KEY: &str = "app";
pub const LAST_LOCATION_KEY: &str = "last_location";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AppSettings {
    /// Re-open the page the user was on when the app was closed.
    pub restore_last_location: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            restore_last_location: true,
        }
    }
}

impl AppSettings {
    /// Clamps values into supported ranges (numeric settings are added by later stages).
    pub fn normalized(self) -> Self {
        self
    }
}

pub fn get_value<T: DeserializeOwned>(conn: &Connection, key: &str) -> AppResult<Option<T>> {
    let raw: Option<String> = conn
        .query_row("SELECT value FROM app_settings WHERE key = ?1", [key], |r| r.get(0))
        .optional()?;
    Ok(match raw {
        // A corrupt value falls back to defaults instead of breaking the app.
        Some(text) => serde_json::from_str(&text).ok(),
        None => None,
    })
}

pub fn set_value<T: Serialize>(conn: &Connection, key: &str, value: &T) -> AppResult<()> {
    conn.execute(
        "INSERT INTO app_settings (key, value, updated_at) VALUES (?1, ?2, datetime('now', 'localtime'))
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        params![key, serde_json::to_string(value)?],
    )?;
    Ok(())
}

pub fn load(conn: &Connection) -> AppResult<AppSettings> {
    Ok(get_value::<AppSettings>(conn, APP_KEY)?.unwrap_or_default().normalized())
}

pub fn save(conn: &Connection, settings: AppSettings) -> AppResult<AppSettings> {
    let settings = settings.normalized();
    set_value(conn, APP_KEY, &settings)?;
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_persist_and_corrupt_values_fall_back() {
        let mut conn = Connection::open_in_memory().unwrap();
        crate::database::migrations::migrate(&mut conn).unwrap();
        assert_eq!(load(&conn).unwrap(), AppSettings::default());
        save(&conn, AppSettings { restore_last_location: false }).unwrap();
        assert!(!load(&conn).unwrap().restore_last_location);
        conn.execute("UPDATE app_settings SET value = '{broken' WHERE key = 'app'", [])
            .unwrap();
        assert_eq!(load(&conn).unwrap(), AppSettings::default());
        // Unknown keys from a newer version and missing keys are tolerated.
        conn.execute("UPDATE app_settings SET value = '{\"future\":1}' WHERE key = 'app'", [])
            .unwrap();
        assert!(load(&conn).unwrap().restore_last_location);
    }
}
