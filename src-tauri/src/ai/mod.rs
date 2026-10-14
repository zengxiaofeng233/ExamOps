//! Model Gateway: user-configured models, HTTP client, streaming, cancellation
//! and token accounting. The model never gets database or system access — its
//! output is only displayed or validated as data by the calling engine.

pub mod client;
pub mod error;
pub mod json;
pub mod profiles;
pub mod sse;
pub mod tokens;

use crate::error::AppResult;
use crate::security::SecretStore;
use client::Gateway;
use rusqlite::{params, Connection};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;

pub struct AiState {
    pub gateway: Gateway,
    pub secrets: Arc<dyn SecretStore>,
    inflight: Mutex<HashMap<String, CancellationToken>>,
}

impl AiState {
    pub fn new(secrets: Arc<dyn SecretStore>) -> Self {
        AiState { gateway: Gateway::new(), secrets, inflight: Mutex::new(HashMap::new()) }
    }

    /// Registers a cancellable request under `id`.
    pub fn register(&self, id: &str) -> CancellationToken {
        let token = CancellationToken::new();
        self.inflight.lock().expect("inflight lock").insert(id.to_string(), token.clone());
        token
    }

    pub fn finish(&self, id: &str) {
        self.inflight.lock().expect("inflight lock").remove(id);
    }

    pub fn cancel(&self, id: &str) -> bool {
        match self.inflight.lock().expect("inflight lock").get(id) {
            Some(token) => {
                token.cancel();
                true
            }
            None => false,
        }
    }
}

/// Removes the in-flight entry when a request ends, however it ends.
pub struct InflightGuard<'a> {
    state: &'a AiState,
    id: String,
}

impl<'a> InflightGuard<'a> {
    pub fn new(state: &'a AiState, id: &str) -> (Self, CancellationToken) {
        let token = state.register(id);
        (InflightGuard { state, id: id.to_string() }, token)
    }
}

impl Drop for InflightGuard<'_> {
    fn drop(&mut self) {
        self.state.finish(&self.id);
    }
}

pub fn record_usage(conn: &Connection, profile_id: Option<i64>, kind: &str, prompt: u64, completion: u64, estimated: bool) -> AppResult<()> {
    conn.execute(
        "INSERT INTO ai_usage (profile_id, kind, prompt_tokens, completion_tokens, estimated) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![profile_id, kind, prompt as i64, completion as i64, estimated],
    )?;
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct UsageSummary {
    pub today_prompt: i64,
    pub today_completion: i64,
    pub total_prompt: i64,
    pub total_completion: i64,
    pub requests: i64,
}

pub fn usage_summary(conn: &Connection) -> AppResult<UsageSummary> {
    Ok(conn.query_row(
        "SELECT
            COALESCE(SUM(CASE WHEN date(created_at) = date('now', 'localtime') THEN prompt_tokens END), 0),
            COALESCE(SUM(CASE WHEN date(created_at) = date('now', 'localtime') THEN completion_tokens END), 0),
            COALESCE(SUM(prompt_tokens), 0), COALESCE(SUM(completion_tokens), 0), COUNT(*)
         FROM ai_usage",
        [],
        |r| Ok(UsageSummary { today_prompt: r.get(0)?, today_completion: r.get(1)?, total_prompt: r.get(2)?, total_completion: r.get(3)?, requests: r.get(4)? }),
    )?)
}
