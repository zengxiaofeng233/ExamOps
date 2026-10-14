//! Errors from AI providers, mapped to messages a student can act on.

#[derive(Debug, Clone, PartialEq)]
pub enum GatewayError {
    /// No model configured / selected.
    NoProfile,
    /// The configured URL is not http(s).
    InvalidUrl(String),
    Network(String),
    Timeout,
    Auth { status: u16, detail: String },
    NotFound { status: u16, detail: String },
    RateLimited { detail: String },
    BadRequest { status: u16, detail: String },
    Server { status: u16, detail: String },
    InvalidResponse(String),
    /// The connection dropped part-way through a streamed answer.
    Interrupted(String),
    Cancelled,
}

impl std::fmt::Display for GatewayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let detail = |d: &str| if d.is_empty() { String::new() } else { format!("：{d}") };
        match self {
            GatewayError::NoProfile => write!(f, "尚未配置 AI 模型，请先在“设置 → AI 模型”中添加接口"),
            GatewayError::InvalidUrl(url) => write!(f, "接口地址无效（{url}），需以 http:// 或 https:// 开头"),
            GatewayError::Network(e) => write!(f, "无法连接到 AI 服务，请检查网络或接口地址{}", detail(e)),
            GatewayError::Timeout => write!(f, "AI 服务响应超时，请稍后重试或在模型设置中延长超时时间"),
            GatewayError::Auth { status, detail: d } => write!(f, "API Key 无效或无权访问该模型（HTTP {status}）{}", detail(d)),
            GatewayError::NotFound { status, detail: d } => write!(f, "接口地址或模型名称不存在（HTTP {status}）{}", detail(d)),
            GatewayError::RateLimited { detail: d } => write!(f, "请求过于频繁或账户额度不足（HTTP 429）{}", detail(d)),
            GatewayError::BadRequest { status, detail: d } => write!(f, "AI 服务拒绝了请求（HTTP {status}）{}", detail(d)),
            GatewayError::Server { status, detail: d } => write!(f, "AI 服务暂时不可用（HTTP {status}）{}", detail(d)),
            GatewayError::InvalidResponse(e) => write!(f, "AI 服务返回了无法识别的数据{}", detail(e)),
            // The transport error ("error decoding response body"…) means nothing to a student;
            // it stays available through `Debug` for diagnostics.
            GatewayError::Interrupted(_) => write!(f, "连接中断，已保留已生成的内容，可以点击“重试”重新生成"),
            GatewayError::Cancelled => write!(f, "已停止生成"),
        }
    }
}

impl std::error::Error for GatewayError {}

impl From<GatewayError> for crate::error::AppError {
    fn from(error: GatewayError) -> Self {
        crate::error::AppError::Message(error.to_string())
    }
}

impl GatewayError {
    /// Builds an error from an HTTP status and response body (provider JSON or text).
    pub fn from_status(status: u16, body: &str) -> Self {
        let detail = provider_message(body);
        match status {
            401 | 403 => GatewayError::Auth { status, detail },
            404 => GatewayError::NotFound { status, detail },
            429 => GatewayError::RateLimited { detail },
            400..=499 => GatewayError::BadRequest { status, detail },
            _ => GatewayError::Server { status, detail },
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            GatewayError::NoProfile => "no_profile",
            GatewayError::InvalidUrl(_) => "invalid_url",
            GatewayError::Network(_) => "network",
            GatewayError::Timeout => "timeout",
            GatewayError::Auth { .. } => "auth",
            GatewayError::NotFound { .. } => "not_found",
            GatewayError::RateLimited { .. } => "rate_limited",
            GatewayError::BadRequest { .. } => "bad_request",
            GatewayError::Server { .. } => "server",
            GatewayError::InvalidResponse(_) => "invalid_response",
            GatewayError::Interrupted(_) => "interrupted",
            GatewayError::Cancelled => "cancelled",
        }
    }
}

/// Extracts `error.message` (OpenAI style) or a short text snippet.
pub fn provider_message(body: &str) -> String {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(body) {
        for pointer in ["/error/message", "/message", "/error", "/detail", "/msg"] {
            if let Some(text) = value.pointer(pointer).and_then(|v| v.as_str()) {
                return truncate(text, 300);
            }
        }
    }
    truncate(body.trim(), 300)
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        format!("{}…", text.chars().take(max).collect::<String>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statuses_map_to_actionable_messages() {
        let auth = GatewayError::from_status(401, r#"{"error":{"message":"Incorrect API key provided"}}"#);
        assert_eq!(auth.kind(), "auth");
        assert!(auth.to_string().contains("Incorrect API key"));
        assert_eq!(GatewayError::from_status(404, "").kind(), "not_found");
        assert_eq!(GatewayError::from_status(429, "quota").kind(), "rate_limited");
        assert_eq!(GatewayError::from_status(422, "").kind(), "bad_request");
        assert_eq!(GatewayError::from_status(503, "<html>").kind(), "server");
        assert_eq!(provider_message(&"x".repeat(500)).chars().count(), 301);
    }
}
