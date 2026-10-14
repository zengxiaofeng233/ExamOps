//! OpenAI-compatible chat completions client: streaming, non-streaming,
//! connection tests, timeouts, cancellation and provider quirks.

use super::error::GatewayError;
use super::profiles::ResolvedProfile;
use super::sse::{self, Chunk, SseDecoder, Usage};
use futures::StreamExt;
use serde::Serialize;
use serde_json::{json, Value};
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        ChatMessage { role: "system".into(), content: content.into() }
    }
    pub fn user(content: impl Into<String>) -> Self {
        ChatMessage { role: "user".into(), content: content.into() }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ChatOptions {
    pub max_tokens: Option<u32>,
    pub temperature: Option<f64>,
    /// Ask for a JSON object (only sent when the profile enables JSON mode).
    pub json: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Completion {
    pub text: String,
    pub reasoning: String,
    pub usage: Option<Usage>,
    pub finish_reason: Option<String>,
    pub elapsed_ms: u64,
}

/// A streamed answer: whatever arrived, plus the error that ended it (if any).
#[derive(Debug, Clone)]
pub struct StreamOutcome {
    pub completion: Completion,
    pub error: Option<GatewayError>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TestReport {
    pub ok: bool,
    pub latency_ms: u64,
    pub reply: String,
    pub model: String,
}

pub enum StreamEvent<'a> {
    Text(&'a str),
    Reasoning(&'a str),
}

/// Parameter variants some providers require (e.g. newer OpenAI models).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Compat {
    max_completion_tokens: bool,
    omit_temperature: bool,
}

impl Compat {
    /// Adjusts parameters after a 400 response that names them; `None` = nothing to try.
    fn adjust(self, body: &str) -> Option<Compat> {
        let lower = body.to_lowercase();
        let mut next = self;
        let unsupported = ["unsupported", "not supported", "does not support", "only the default", "not allowed"]
            .iter()
            .any(|w| lower.contains(w));
        if unsupported && lower.contains("max_tokens") {
            next.max_completion_tokens = true;
        }
        if unsupported && lower.contains("temperature") {
            next.omit_temperature = true;
        }
        (next != self).then_some(next)
    }
}

#[derive(Clone)]
pub struct Gateway {
    http: reqwest::Client,
}

impl Default for Gateway {
    fn default() -> Self {
        Self::new()
    }
}

impl Gateway {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .user_agent(concat!("ExamOps/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("HTTP client");
        Gateway { http }
    }

    fn body(profile: &ResolvedProfile, messages: &[ChatMessage], options: &ChatOptions, stream: bool, compat: Compat) -> Value {
        let mut body = json!({
            "model": profile.model,
            "messages": messages,
            "stream": stream,
        });
        let max_tokens = options.max_tokens.unwrap_or(profile.max_output_tokens);
        body[if compat.max_completion_tokens { "max_completion_tokens" } else { "max_tokens" }] = json!(max_tokens);
        if !compat.omit_temperature {
            body["temperature"] = json!(options.temperature.unwrap_or(profile.temperature));
            if let Some(top_p) = profile.top_p {
                body["top_p"] = json!(top_p);
            }
        }
        if options.json && profile.json_mode {
            body["response_format"] = json!({ "type": "json_object" });
        }
        body
    }

    async fn post(&self, profile: &ResolvedProfile, body: &Value, stream: bool) -> Result<reqwest::Response, GatewayError> {
        let mut request = self.http.post(&profile.endpoint).json(body);
        if let Some(key) = profile.api_key.as_deref().filter(|k| !k.is_empty()) {
            request = request.bearer_auth(key);
        }
        if stream {
            request = request.header(reqwest::header::ACCEPT, "text/event-stream");
        } else {
            request = request.timeout(Duration::from_secs(profile.timeout_secs));
        }
        request.send().await.map_err(map_reqwest)
    }

    /// Sends a request, retrying once with adjusted parameters if the provider rejects them.
    async fn send(
        &self,
        profile: &ResolvedProfile,
        messages: &[ChatMessage],
        options: &ChatOptions,
        stream: bool,
        cancel: &CancellationToken,
    ) -> Result<reqwest::Response, GatewayError> {
        let mut compat = Compat::default();
        for attempt in 0..2 {
            let body = Self::body(profile, messages, options, stream, compat);
            let response = tokio::select! {
                _ = cancel.cancelled() => return Err(GatewayError::Cancelled),
                response = self.post(profile, &body, stream) => response?,
            };
            let status = response.status();
            if status.is_success() {
                return Ok(response);
            }
            let text = response.text().await.unwrap_or_default();
            if status.as_u16() == 400 && attempt == 0 {
                if let Some(next) = compat.adjust(&text) {
                    compat = next;
                    continue;
                }
            }
            return Err(GatewayError::from_status(status.as_u16(), &text));
        }
        unreachable!("loop returns")
    }

    /// Streams an answer, calling `on_event` for every text / reasoning delta.
    pub async fn stream(
        &self,
        profile: &ResolvedProfile,
        messages: &[ChatMessage],
        options: &ChatOptions,
        cancel: &CancellationToken,
        mut on_event: impl FnMut(StreamEvent<'_>),
    ) -> StreamOutcome {
        let started = Instant::now();
        let mut completion = Completion::default();
        let finish = |mut completion: Completion, error: Option<GatewayError>| {
            completion.elapsed_ms = started.elapsed().as_millis() as u64;
            StreamOutcome { completion, error }
        };
        let response = match self.send(profile, messages, options, true, cancel).await {
            Ok(response) => response,
            Err(error) => return finish(completion, Some(error)),
        };
        let is_sse = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|v| v.contains("event-stream"))
            .unwrap_or(true);
        if !is_sse {
            // The server ignored `stream: true` and answered in one piece.
            let text = response.text().await.unwrap_or_default();
            return match parse_completion(&text) {
                Ok(full) => {
                    on_event(StreamEvent::Text(&full.text));
                    finish(full, None)
                }
                Err(error) => finish(completion, Some(error)),
            };
        }
        let idle = Duration::from_secs(profile.timeout_secs);
        let mut bytes = response.bytes_stream();
        let mut decoder = SseDecoder::default();
        let mut done = false;
        loop {
            let next = tokio::select! {
                _ = cancel.cancelled() => return finish(completion, Some(GatewayError::Cancelled)),
                next = tokio::time::timeout(idle, bytes.next()) => next,
            };
            let ended = matches!(next, Ok(None));
            let payloads = match next {
                Err(_) => return finish(completion, Some(GatewayError::Timeout)),
                Ok(None) => decoder.finish(),
                Ok(Some(Err(error))) => {
                    let error = if completion.text.is_empty() { map_reqwest(error) } else { GatewayError::Interrupted(error.to_string()) };
                    return finish(completion, Some(error));
                }
                Ok(Some(Ok(chunk))) => decoder.push(&chunk),
            };
            for payload in payloads {
                for chunk in sse::interpret(&payload) {
                    match chunk {
                        Chunk::Text(text) => {
                            on_event(StreamEvent::Text(&text));
                            completion.text.push_str(&text);
                        }
                        Chunk::Reasoning(text) => {
                            on_event(StreamEvent::Reasoning(&text));
                            completion.reasoning.push_str(&text);
                        }
                        Chunk::Usage(usage) => completion.usage = Some(usage),
                        Chunk::Finish(reason) => completion.finish_reason = Some(reason),
                        Chunk::Error(message) => {
                            let error = if completion.text.is_empty() {
                                GatewayError::Server { status: 200, detail: message }
                            } else {
                                GatewayError::Interrupted(message)
                            };
                            return finish(completion, Some(error));
                        }
                        Chunk::Done => done = true,
                    }
                }
            }
            if done || ended {
                break;
            }
        }
        if completion.text.is_empty() && completion.reasoning.is_empty() {
            return finish(completion, Some(GatewayError::InvalidResponse("没有收到任何回答内容".into())));
        }
        if !done && completion.finish_reason.is_none() && completion.text.is_empty() {
            return finish(completion, Some(GatewayError::Interrupted(String::new())));
        }
        finish(completion, None)
    }

    /// Non-streaming request for structured tasks (quiz generation, grading...).
    pub async fn complete(
        &self,
        profile: &ResolvedProfile,
        messages: &[ChatMessage],
        options: &ChatOptions,
        cancel: &CancellationToken,
    ) -> Result<Completion, GatewayError> {
        let started = Instant::now();
        let mut attempt = 0;
        loop {
            let result = async {
                let response = self.send(profile, messages, options, false, cancel).await?;
                let text = tokio::select! {
                    _ = cancel.cancelled() => return Err(GatewayError::Cancelled),
                    text = response.text() => text.map_err(map_reqwest)?,
                };
                parse_completion(&text)
            }
            .await;
            match result {
                Ok(mut completion) => {
                    completion.elapsed_ms = started.elapsed().as_millis() as u64;
                    return Ok(completion);
                }
                Err(error) => {
                    // One retry for transient failures (network, rate limit, 5xx).
                    let retryable = matches!(error, GatewayError::Network(_) | GatewayError::RateLimited { .. })
                        || matches!(error, GatewayError::Server { status, .. } if (500..600).contains(&status));
                    if attempt > 0 || !retryable {
                        return Err(error);
                    }
                    attempt += 1;
                    tokio::select! {
                        _ = cancel.cancelled() => return Err(GatewayError::Cancelled),
                        _ = tokio::time::sleep(Duration::from_millis(1500)) => {}
                    }
                }
            }
        }
    }

    /// Sends a tiny request to check URL, key and model.
    pub async fn test(&self, profile: &ResolvedProfile) -> Result<TestReport, GatewayError> {
        let started = Instant::now();
        let messages = [ChatMessage::user("请只回复两个字：你好")];
        let options = ChatOptions { max_tokens: Some(16), temperature: Some(0.0), json: false };
        let cancel = CancellationToken::new();
        let completion = self.complete(profile, &messages, &options, &cancel).await?;
        Ok(TestReport {
            ok: true,
            latency_ms: started.elapsed().as_millis() as u64,
            reply: completion.text.chars().take(80).collect(),
            model: profile.model.clone(),
        })
    }
}

fn map_reqwest(error: reqwest::Error) -> GatewayError {
    if error.is_timeout() {
        GatewayError::Timeout
    } else if error.is_decode() || error.is_body() {
        GatewayError::InvalidResponse(error.to_string())
    } else {
        let mut message = error.to_string();
        let mut source = std::error::Error::source(&error);
        while let Some(inner) = source {
            message = format!("{message}（{inner}）");
            source = inner.source();
        }
        GatewayError::Network(message)
    }
}

/// Parses a non-streamed chat completion body.
pub fn parse_completion(body: &str) -> Result<Completion, GatewayError> {
    let value: Value = serde_json::from_str(body).map_err(|_| GatewayError::InvalidResponse(super::error::provider_message(body)))?;
    if let Some(error) = value.get("error").filter(|e| !e.is_null()) {
        let message = error.get("message").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| error.to_string());
        return Err(GatewayError::Server { status: 200, detail: message });
    }
    let choice = value
        .get("choices")
        .and_then(|c| c.get(0))
        .ok_or_else(|| GatewayError::InvalidResponse("缺少 choices 字段".into()))?;
    let message = choice.get("message").cloned().unwrap_or(Value::Null);
    let text = message.get("content").and_then(Value::as_str).unwrap_or("").to_string();
    let reasoning = message
        .get("reasoning_content")
        .or_else(|| message.get("reasoning"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if text.is_empty() && reasoning.is_empty() {
        return Err(GatewayError::InvalidResponse("回答内容为空".into()));
    }
    Ok(Completion {
        text,
        reasoning,
        usage: sse::usage_of(&value),
        finish_reason: choice.get("finish_reason").and_then(Value::as_str).map(str::to_string),
        elapsed_ms: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compat_adjusts_only_named_parameters() {
        let c = Compat::default();
        let next = c.adjust("Unsupported parameter: 'max_tokens' is not supported with this model. Use 'max_completion_tokens' instead.").unwrap();
        assert!(next.max_completion_tokens && !next.omit_temperature);
        let next = next.adjust("Unsupported value: 'temperature' does not support 0.3 with this model. Only the default (1) value is supported.").unwrap();
        assert!(next.omit_temperature);
        assert!(next.adjust("some other error").is_none());
        let profile = super::super::profiles::ResolvedProfile {
            id: None, name: "t".into(), endpoint: "http://x/chat/completions".into(), model: "m".into(), api_key: None,
            temperature: 0.3, top_p: Some(0.9), max_output_tokens: 512, context_window: 8000, timeout_secs: 30, json_mode: true,
        };
        let body = Gateway::body(&profile, &[ChatMessage::user("hi")], &ChatOptions { json: true, ..Default::default() }, true, next);
        assert!(body.get("max_tokens").is_none() && body["max_completion_tokens"] == 512);
        assert!(body.get("temperature").is_none() && body.get("top_p").is_none());
        assert_eq!(body["response_format"]["type"], "json_object");
    }

    #[test]
    fn parses_completions_and_errors() {
        let ok = parse_completion(r#"{"choices":[{"message":{"content":"你好"},"finish_reason":"stop"}],"usage":{"prompt_tokens":3,"completion_tokens":2}}"#).unwrap();
        assert_eq!((ok.text.as_str(), ok.finish_reason.as_deref()), ("你好", Some("stop")));
        assert_eq!(ok.usage.unwrap().completion_tokens, 2);
        assert!(matches!(parse_completion("<html>bad gateway</html>"), Err(GatewayError::InvalidResponse(_))));
        assert!(matches!(parse_completion(r#"{"error":{"message":"quota"}}"#), Err(GatewayError::Server { .. })));
        assert!(matches!(parse_completion(r#"{"choices":[{"message":{"content":""}}]}"#), Err(GatewayError::InvalidResponse(_))));
    }
}
