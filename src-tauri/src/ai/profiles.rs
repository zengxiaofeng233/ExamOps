//! User-configured AI models (BYOK). Keys live in the OS credential store.

use super::error::GatewayError;
use crate::error::{AppError, AppResult, OptionalRow};
use crate::security::{key_hint, SecretStore};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct ModelProfile {
    pub id: i64,
    pub name: String,
    pub base_url: String,
    pub model: String,
    pub temperature: f64,
    pub top_p: Option<f64>,
    pub max_output_tokens: i64,
    pub context_window: i64,
    pub timeout_secs: i64,
    pub json_mode: bool,
    pub is_default: bool,
    pub has_key: bool,
    pub key_storage: String,
    pub key_hint: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProfileInput {
    pub id: Option<i64>,
    pub name: String,
    pub base_url: String,
    pub model: String,
    pub temperature: f64,
    pub top_p: Option<f64>,
    pub max_output_tokens: i64,
    pub context_window: i64,
    pub timeout_secs: i64,
    #[serde(default)]
    pub json_mode: bool,
    /// `None` keeps the stored key, `Some("")` removes it.
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SaveResult {
    pub profile: ModelProfile,
    pub warning: Option<String>,
}

/// Everything needed to call the API, including the secret.
#[derive(Clone)]
pub struct ResolvedProfile {
    pub id: Option<i64>,
    pub name: String,
    pub endpoint: String,
    pub model: String,
    pub api_key: Option<String>,
    pub temperature: f64,
    pub top_p: Option<f64>,
    pub max_output_tokens: u32,
    pub context_window: usize,
    pub timeout_secs: u64,
    pub json_mode: bool,
}

impl std::fmt::Debug for ResolvedProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never print the key.
        f.debug_struct("ResolvedProfile").field("name", &self.name).field("endpoint", &self.endpoint).field("model", &self.model).finish()
    }
}

const COLUMNS: &str = "id, name, base_url, model, temperature, top_p, max_output_tokens, context_window, timeout_secs,
    json_mode, is_default, key_storage, key_hint, created_at, updated_at";

fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ModelProfile> {
    let key_storage: String = row.get(11)?;
    Ok(ModelProfile {
        id: row.get(0)?,
        name: row.get(1)?,
        base_url: row.get(2)?,
        model: row.get(3)?,
        temperature: row.get(4)?,
        top_p: row.get(5)?,
        max_output_tokens: row.get(6)?,
        context_window: row.get(7)?,
        timeout_secs: row.get(8)?,
        json_mode: row.get(9)?,
        is_default: row.get(10)?,
        has_key: key_storage != "none",
        key_storage,
        key_hint: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
    })
}

fn secret_name(id: i64) -> String {
    format!("model-profile-{id}")
}

/// `https://host/v1` → `https://host/v1/chat/completions` (full paths are kept).
pub fn endpoint(base_url: &str) -> Result<String, GatewayError> {
    let trimmed = base_url.trim().trim_end_matches('/');
    let parsed = url::Url::parse(trimmed).map_err(|_| GatewayError::InvalidUrl(trimmed.to_string()))?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err(GatewayError::InvalidUrl(trimmed.to_string()));
    }
    if trimmed.ends_with("/chat/completions") {
        Ok(trimmed.to_string())
    } else {
        Ok(format!("{trimmed}/chat/completions"))
    }
}

pub fn validate(input: &ProfileInput) -> AppResult<()> {
    if input.name.trim().is_empty() {
        return Err(AppError::invalid("请填写配置名称"));
    }
    if input.model.trim().is_empty() {
        return Err(AppError::invalid("请填写模型名称"));
    }
    endpoint(&input.base_url).map_err(|e| AppError::invalid(e.to_string()))?;
    let checks = [
        ((0.0..=2.0).contains(&input.temperature), "温度需在 0–2 之间"),
        (input.top_p.map(|p| (0.0..=1.0).contains(&p)).unwrap_or(true), "Top P 需在 0–1 之间"),
        ((64..=65536).contains(&input.max_output_tokens), "最大输出 tokens 需在 64–65536 之间"),
        ((2048..=2_000_000).contains(&input.context_window), "上下文预算需在 2048–2000000 tokens 之间"),
        ((10..=900).contains(&input.timeout_secs), "超时时间需在 10–900 秒之间"),
        (input.max_output_tokens < input.context_window, "最大输出 tokens 必须小于上下文预算"),
    ];
    for (ok, message) in checks {
        if !ok {
            return Err(AppError::invalid(message));
        }
    }
    Ok(())
}

pub fn list(conn: &Connection) -> AppResult<Vec<ModelProfile>> {
    let mut stmt = conn.prepare(&format!("SELECT {COLUMNS} FROM model_profiles ORDER BY is_default DESC, id"))?;
    let rows = stmt.query_map([], from_row)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn get(conn: &Connection, id: i64) -> AppResult<ModelProfile> {
    conn.query_row(&format!("SELECT {COLUMNS} FROM model_profiles WHERE id = ?1"), [id], from_row)
        .or_not_found("模型配置")
}

pub fn save(conn: &mut Connection, store: &dyn SecretStore, input: ProfileInput) -> AppResult<SaveResult> {
    validate(&input)?;
    let tx = conn.transaction()?;
    let id = match input.id {
        Some(id) => {
            let updated = tx.execute(
                "UPDATE model_profiles SET name = ?1, base_url = ?2, model = ?3, temperature = ?4, top_p = ?5,
                 max_output_tokens = ?6, context_window = ?7, timeout_secs = ?8, json_mode = ?9,
                 updated_at = datetime('now', 'localtime') WHERE id = ?10",
                params![input.name.trim(), input.base_url.trim(), input.model.trim(), input.temperature, input.top_p,
                        input.max_output_tokens, input.context_window, input.timeout_secs, input.json_mode, id],
            )?;
            if updated == 0 {
                return Err(AppError::NotFound("模型配置"));
            }
            id
        }
        None => {
            let first: bool = tx.query_row("SELECT NOT EXISTS(SELECT 1 FROM model_profiles)", [], |r| r.get(0))?;
            tx.execute(
                "INSERT INTO model_profiles (name, base_url, model, temperature, top_p, max_output_tokens, context_window,
                 timeout_secs, json_mode, is_default) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![input.name.trim(), input.base_url.trim(), input.model.trim(), input.temperature, input.top_p,
                        input.max_output_tokens, input.context_window, input.timeout_secs, input.json_mode, first],
            )?;
            tx.last_insert_rowid()
        }
    };
    let mut warning = None;
    if let Some(key) = input.api_key.as_deref().map(str::trim) {
        let _ = store.delete(&secret_name(id));
        if key.is_empty() {
            tx.execute(
                "UPDATE model_profiles SET key_storage = 'none', api_key_local = NULL, key_hint = NULL WHERE id = ?1",
                [id],
            )?;
        } else {
            let (storage, local) = match store.set(&secret_name(id), key) {
                Ok(()) => ("keyring", None),
                Err(error) => {
                    warning = Some(format!(
                        "{}不可用（{error}），API Key 已改为保存在本机数据库中。请勿把数据目录分享给他人。",
                        store.label()
                    ));
                    ("local", Some(key.to_string()))
                }
            };
            tx.execute(
                "UPDATE model_profiles SET key_storage = ?1, api_key_local = ?2, key_hint = ?3 WHERE id = ?4",
                params![storage, local, key_hint(key), id],
            )?;
        }
    }
    tx.commit()?;
    Ok(SaveResult { profile: get(conn, id)?, warning })
}

pub fn delete(conn: &mut Connection, store: &dyn SecretStore, id: i64) -> AppResult<()> {
    let was_default: bool = conn
        .query_row("SELECT is_default FROM model_profiles WHERE id = ?1", [id], |r| r.get(0))
        .or_not_found("模型配置")?;
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM model_profiles WHERE id = ?1", [id])?;
    if was_default {
        tx.execute("UPDATE model_profiles SET is_default = 1 WHERE id = (SELECT MIN(id) FROM model_profiles)", [])?;
    }
    tx.commit()?;
    let _ = store.delete(&secret_name(id));
    Ok(())
}

pub fn set_default(conn: &Connection, id: i64) -> AppResult<()> {
    get(conn, id)?;
    conn.execute("UPDATE model_profiles SET is_default = (id = ?1)", [id])?;
    Ok(())
}

/// Picks the requested profile, else the default one, and loads its key.
pub fn resolve(conn: &Connection, store: &dyn SecretStore, id: Option<i64>) -> Result<ResolvedProfile, AppError> {
    let chosen: Option<i64> = match id {
        Some(id) => conn.query_row("SELECT id FROM model_profiles WHERE id = ?1", [id], |r| r.get(0)).optional()?,
        None => None,
    };
    let chosen = match chosen {
        Some(id) => Some(id),
        None => conn
            .query_row("SELECT id FROM model_profiles ORDER BY is_default DESC, id LIMIT 1", [], |r| r.get(0))
            .optional()?,
    };
    let Some(id) = chosen else {
        return Err(GatewayError::NoProfile.into());
    };
    let (storage, local): (String, Option<String>) = conn.query_row(
        "SELECT key_storage, api_key_local FROM model_profiles WHERE id = ?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let api_key = match storage.as_str() {
        "keyring" => Some(store.get(&secret_name(id)).map_err(|e| AppError::msg(format!("无法读取 API Key：{e}")))?
            .ok_or_else(|| AppError::msg("系统凭据中找不到该模型的 API Key，请在设置中重新填写"))?),
        "local" => local,
        _ => None,
    };
    let profile = get(conn, id)?;
    to_resolved(&profile, api_key)
}

pub fn to_resolved(profile: &ModelProfile, api_key: Option<String>) -> Result<ResolvedProfile, AppError> {
    Ok(ResolvedProfile {
        id: Some(profile.id),
        name: profile.name.clone(),
        endpoint: endpoint(&profile.base_url)?,
        model: profile.model.clone(),
        api_key,
        temperature: profile.temperature,
        top_p: profile.top_p,
        max_output_tokens: profile.max_output_tokens.max(64) as u32,
        context_window: profile.context_window.max(2048) as usize,
        timeout_secs: profile.timeout_secs.max(10) as u64,
        json_mode: profile.json_mode,
    })
}

/// A profile built from unsaved form values (for "test connection" before saving).
pub fn from_input(input: &ProfileInput, api_key: Option<String>) -> Result<ResolvedProfile, AppError> {
    validate(input)?;
    Ok(ResolvedProfile {
        id: input.id,
        name: input.name.clone(),
        endpoint: endpoint(&input.base_url)?,
        model: input.model.trim().to_string(),
        api_key,
        temperature: input.temperature,
        top_p: input.top_p,
        max_output_tokens: input.max_output_tokens as u32,
        context_window: input.context_window as usize,
        timeout_secs: input.timeout_secs as u64,
        json_mode: input.json_mode,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::MemoryStore;

    fn input(name: &str) -> ProfileInput {
        ProfileInput {
            id: None,
            name: name.into(),
            base_url: "https://api.example.com/v1/".into(),
            model: "test-model".into(),
            temperature: 0.3,
            top_p: None,
            max_output_tokens: 1024,
            context_window: 32000,
            timeout_secs: 60,
            json_mode: false,
            api_key: Some("sk-test-1234567890".into()),
        }
    }

    fn db() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        crate::database::migrations::migrate(&mut conn).unwrap();
        conn
    }

    #[test]
    fn endpoints_are_normalized_and_validated() {
        assert_eq!(endpoint("https://api.deepseek.com/v1/").unwrap(), "https://api.deepseek.com/v1/chat/completions");
        assert_eq!(endpoint("http://localhost:11434/v1/chat/completions").unwrap(), "http://localhost:11434/v1/chat/completions");
        assert!(endpoint("ftp://x").is_err());
        assert!(endpoint("not a url").is_err());
    }

    #[test]
    fn keys_go_to_the_secret_store_and_never_to_the_profile() {
        let mut conn = db();
        let store = MemoryStore::default();
        let saved = save(&mut conn, &store, input("DeepSeek")).unwrap();
        assert!(saved.warning.is_none());
        assert!(saved.profile.is_default && saved.profile.has_key);
        assert_eq!(saved.profile.key_storage, "keyring");
        assert_eq!(saved.profile.key_hint.as_deref(), Some("sk-t…7890"));
        let local: Option<String> = conn.query_row("SELECT api_key_local FROM model_profiles", [], |r| r.get(0)).unwrap();
        assert!(local.is_none());
        let resolved = resolve(&conn, &store, None).unwrap();
        assert_eq!(resolved.api_key.as_deref(), Some("sk-test-1234567890"));
        assert!(!format!("{resolved:?}").contains("sk-test"));
        // Editing without a key keeps it; an empty key removes it.
        let mut edit = input("DeepSeek 2");
        edit.id = Some(saved.profile.id);
        edit.api_key = None;
        assert!(save(&mut conn, &store, edit.clone()).unwrap().profile.has_key);
        edit.api_key = Some(String::new());
        assert!(!save(&mut conn, &store, edit).unwrap().profile.has_key);
        assert!(resolve(&conn, &store, None).unwrap().api_key.is_none());
    }

    #[test]
    fn unavailable_store_falls_back_with_a_warning() {
        let mut conn = db();
        let store = MemoryStore::failing();
        let saved = save(&mut conn, &store, input("本地")).unwrap();
        assert!(saved.warning.unwrap().contains("本机数据库"));
        assert_eq!(saved.profile.key_storage, "local");
        assert_eq!(resolve(&conn, &store, None).unwrap().api_key.as_deref(), Some("sk-test-1234567890"));
    }

    #[test]
    fn defaults_follow_deletion_and_validation_rejects_bad_values() {
        let mut conn = db();
        let store = MemoryStore::default();
        let first = save(&mut conn, &store, input("A")).unwrap().profile;
        let second = save(&mut conn, &store, input("B")).unwrap().profile;
        assert!(!second.is_default);
        set_default(&conn, second.id).unwrap();
        assert!(get(&conn, second.id).unwrap().is_default && !get(&conn, first.id).unwrap().is_default);
        delete(&mut conn, &store, second.id).unwrap();
        assert!(get(&conn, first.id).unwrap().is_default);
        let mut bad = input("C");
        bad.temperature = 3.0;
        assert!(save(&mut conn, &store, bad).is_err());
        let mut bad = input("D");
        bad.max_output_tokens = 40000;
        assert!(save(&mut conn, &store, bad).unwrap_err().to_string().contains("小于"));
        delete(&mut conn, &store, first.id).unwrap();
        assert_eq!(resolve(&conn, &store, None).unwrap_err().to_string(), GatewayError::NoProfile.to_string());
    }
}
