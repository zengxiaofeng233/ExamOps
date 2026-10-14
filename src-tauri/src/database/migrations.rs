//! Versioned schema migrations.
//!
//! The schema version is stored in `PRAGMA user_version`. Each migration runs
//! in its own transaction together with the version bump, so a failure leaves
//! the database at the previous version. Applied migrations are also logged
//! in `schema_migrations` for diagnostics.
//!
//! Rule for contributors: never edit a released migration file. Add a new
//! numbered file under `src-tauri/migrations/` and register it below.

use rusqlite::{params, Connection};

pub struct Migration {
    pub version: u32,
    pub name: &'static str,
    pub sql: &'static str,
}

pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "baseline",
        sql: include_str!("../../migrations/0001_baseline.sql"),
    },
    Migration {
        version: 2,
        name: "app_settings",
        sql: include_str!("../../migrations/0002_app_settings.sql"),
    },
];

pub fn latest_version() -> u32 {
    MIGRATIONS.last().map(|m| m.version).unwrap_or(0)
}

pub fn current_version(conn: &Connection) -> rusqlite::Result<u32> {
    conn.pragma_query_value(None, "user_version", |row| row.get(0))
}

/// Brings the database up to the latest schema version.
/// Returns the list of versions that were applied.
pub fn migrate(conn: &mut Connection) -> Result<Vec<u32>, String> {
    let current = current_version(conn).map_err(|e| e.to_string())?;
    let latest = latest_version();
    if current > latest {
        return Err(format!(
            "数据库版本（{current}）高于当前软件支持的版本（{latest}），请升级 ExamOps 后再打开"
        ));
    }
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            applied_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
        )",
    )
    .map_err(|e| e.to_string())?;

    let mut applied = Vec::new();
    for migration in MIGRATIONS.iter().filter(|m| m.version > current) {
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        tx.execute_batch(migration.sql).map_err(|e| {
            format!("数据库迁移 {:04}_{} 失败：{e}", migration.version, migration.name)
        })?;
        tx.execute(
            "INSERT OR REPLACE INTO schema_migrations (version, name) VALUES (?1, ?2)",
            params![migration.version, migration.name],
        )
        .map_err(|e| e.to_string())?;
        tx.pragma_update(None, "user_version", migration.version)
            .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        applied.push(migration.version);
    }
    Ok(applied)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table_exists(conn: &Connection, name: &str) -> bool {
        conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE name = ?1",
            [name],
            |r| r.get::<_, i64>(0),
        )
        .unwrap()
            > 0
    }

    #[test]
    fn versions_are_strictly_increasing() {
        let mut previous = 0;
        for migration in MIGRATIONS {
            assert!(migration.version > previous, "{} out of order", migration.name);
            previous = migration.version;
        }
    }

    #[test]
    fn fresh_database_reaches_latest_and_rerun_is_noop() {
        let mut conn = Connection::open_in_memory().unwrap();
        let applied = migrate(&mut conn).unwrap();
        assert_eq!(applied.len(), MIGRATIONS.len());
        assert_eq!(current_version(&conn).unwrap(), latest_version());
        assert!(migrate(&mut conn).unwrap().is_empty());
        assert!(table_exists(&conn, "courses"));
        assert!(table_exists(&conn, "app_settings"));
    }

    #[test]
    fn phase_one_database_is_upgraded_without_losing_data() {
        let mut conn = Connection::open_in_memory().unwrap();
        // Simulate the unversioned phase-one schema with user data.
        conn.execute_batch(MIGRATIONS[0].sql).unwrap();
        conn.execute("INSERT INTO courses (name, description) VALUES ('高等数学', '期末')", [])
            .unwrap();
        assert_eq!(current_version(&conn).unwrap(), 0);
        migrate(&mut conn).unwrap();
        assert_eq!(current_version(&conn).unwrap(), latest_version());
        let name: String = conn
            .query_row("SELECT name FROM courses", [], |r| r.get(0))
            .unwrap();
        assert_eq!(name, "高等数学");
    }

    #[test]
    fn newer_database_is_rejected() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "user_version", latest_version() + 1).unwrap();
        assert!(migrate(&mut conn).unwrap_err().contains("高于"));
    }
}
