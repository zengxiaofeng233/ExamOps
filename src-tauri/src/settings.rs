//! User preferences stored in the `app_settings` table as JSON values.
//!
//! Every field has a default, so settings saved by an older version keep
//! working when new fields are added.

use crate::context::ContextMode;
use crate::error::AppResult;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

const APP_KEY: &str = "app";
pub const LAST_LOCATION_KEY: &str = "last_location";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ContextSettings {
    /// Default context strategy for new conversations.
    pub mode: ContextMode,
    /// Chunks retrieved per question in RAG mode.
    pub rag_top_k: usize,
    /// Full context is used when the material needs at most this share of the budget.
    pub full_ratio: f64,
    /// Previous question/answer pairs sent with each request.
    pub history_turns: usize,
}

impl Default for ContextSettings {
    fn default() -> Self {
        ContextSettings { mode: ContextMode::Auto, rag_top_k: 8, full_ratio: 0.8, history_turns: 6 }
    }
}

/// When the tutor suggests a quiz (Roadmap · Assessment Engine).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum QuizTrigger {
    /// After the knowledge points of a lesson part have been taught.
    Point,
    /// After a whole chapter has been taught.
    #[default]
    Chapter,
    /// Only when the student starts one.
    Manual,
}

pub const QUESTION_KINDS: [&str; 5] = ["single_choice", "true_false", "short_answer", "recall", "scenario"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct QuizSettings {
    pub trigger: QuizTrigger,
    /// Questions per quiz.
    pub question_count: usize,
    /// Whether a question may be skipped (skipped questions count as wrong).
    pub allow_skip: bool,
    /// Question types used by default.
    pub kinds: Vec<String>,
}

impl Default for QuizSettings {
    fn default() -> Self {
        QuizSettings {
            trigger: QuizTrigger::Chapter,
            question_count: 5,
            allow_skip: true,
            kinds: QUESTION_KINDS.iter().map(|k| k.to_string()).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AppSettings {
    /// Re-open the page the user was on when the app was closed.
    pub restore_last_location: bool,
    /// Try local DirectML acceleration; fall back to CPU if unavailable.
    pub ocr_gpu: bool,
    pub context: ContextSettings,
    pub quiz: QuizSettings,
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings { restore_last_location: true, ocr_gpu: true, context: ContextSettings::default(), quiz: QuizSettings::default() }
    }
}

impl AppSettings {
    /// Clamps values into supported ranges.
    pub fn normalized(mut self) -> Self {
        self.context.rag_top_k = self.context.rag_top_k.clamp(2, 30);
        self.context.full_ratio = if self.context.full_ratio.is_finite() {
            self.context.full_ratio.clamp(0.2, 1.0)
        } else {
            0.8
        };
        self.context.history_turns = self.context.history_turns.min(20);
        self.quiz.question_count = self.quiz.question_count.clamp(1, 20);
        self.quiz.kinds.retain(|k| QUESTION_KINDS.contains(&k.as_str()));
        self.quiz.kinds.dedup();
        if self.quiz.kinds.is_empty() {
            self.quiz.kinds = QuizSettings::default().kinds;
        }
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
        save(&conn, AppSettings { restore_last_location: false, ..Default::default() }).unwrap();
        assert!(!load(&conn).unwrap().restore_last_location);
        conn.execute("UPDATE app_settings SET value = '{broken' WHERE key = 'app'", [])
            .unwrap();
        assert_eq!(load(&conn).unwrap(), AppSettings::default());
        // Unknown keys from a newer version and missing keys are tolerated.
        conn.execute("UPDATE app_settings SET value = '{\"future\":1}' WHERE key = 'app'", [])
            .unwrap();
        assert!(load(&conn).unwrap().restore_last_location);
    }

    #[test]
    fn values_are_clamped() {
        let settings = AppSettings {
            context: ContextSettings { mode: ContextMode::Rag, rag_top_k: 999, full_ratio: f64::NAN, history_turns: 99 },
            quiz: QuizSettings { trigger: QuizTrigger::Point, question_count: 0, allow_skip: false, kinds: vec!["essay".into()] },
            ..Default::default()
        }
        .normalized();
        assert_eq!((settings.context.rag_top_k, settings.context.full_ratio, settings.context.history_turns), (30, 0.8, 20));
        assert_eq!((settings.quiz.question_count, settings.quiz.kinds.len()), (1, 5));
        // Settings saved before quizzes existed get the defaults.
        let old: AppSettings = serde_json::from_str(r#"{"restore_last_location":false}"#).unwrap();
        assert_eq!(old.quiz.trigger, QuizTrigger::Chapter);
    }
}
