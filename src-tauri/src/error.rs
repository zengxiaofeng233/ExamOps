//! Unified error type returned by every Tauri command.
//!
//! Commands return `AppResult<T>`; the error is serialized as a plain
//! user-facing (Chinese) message so the frontend can show it directly.

use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("数据库错误：{0}")]
    Db(#[from] rusqlite::Error),
    #[error("文件操作失败：{0}")]
    Io(#[from] std::io::Error),
    #[error("数据格式错误：{0}")]
    Json(#[from] serde_json::Error),
    #[error("未找到{0}")]
    NotFound(&'static str),
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Message(String),
}

impl AppError {
    pub fn invalid(message: impl Into<String>) -> Self {
        AppError::Invalid(message.into())
    }

    pub fn msg(message: impl Into<String>) -> Self {
        AppError::Message(message.into())
    }
}

impl From<String> for AppError {
    fn from(message: String) -> Self {
        AppError::Message(message)
    }
}

impl From<&str> for AppError {
    fn from(message: &str) -> Self {
        AppError::Message(message.to_string())
    }
}

impl From<tauri::Error> for AppError {
    fn from(error: tauri::Error) -> Self {
        AppError::Message(error.to_string())
    }
}

impl From<tokio::task::JoinError> for AppError {
    fn from(error: tokio::task::JoinError) -> Self {
        AppError::Message(format!("后台任务异常：{error}"))
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;

/// Maps "no row" lookups to a friendly not-found error.
pub trait OptionalRow<T> {
    fn or_not_found(self, what: &'static str) -> AppResult<T>;
}

impl<T> OptionalRow<T> for rusqlite::Result<T> {
    fn or_not_found(self, what: &'static str) -> AppResult<T> {
        match self {
            Ok(value) => Ok(value),
            Err(rusqlite::Error::QueryReturnedNoRows) => Err(AppError::NotFound(what)),
            Err(error) => Err(AppError::Db(error)),
        }
    }
}
