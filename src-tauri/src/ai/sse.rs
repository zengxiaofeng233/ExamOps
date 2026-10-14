//! Server-sent events decoding for OpenAI-compatible streaming responses.

use serde::Serialize;
use serde_json::Value;

/// Incremental SSE decoder: feed raw bytes, receive complete `data` payloads.
#[derive(Default)]
pub struct SseDecoder {
    buffer: Vec<u8>,
    pending: Vec<String>,
}

impl SseDecoder {
    pub fn push(&mut self, bytes: &[u8]) -> Vec<String> {
        self.buffer.extend_from_slice(bytes);
        let mut out = Vec::new();
        while let Some(pos) = self.buffer.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = self.buffer.drain(..=pos).collect();
            let line = String::from_utf8_lossy(&line);
            self.line(line.trim_end_matches(['\n', '\r']), &mut out);
        }
        out
    }

    pub fn finish(&mut self) -> Vec<String> {
        let mut out = Vec::new();
        if !self.buffer.is_empty() {
            let rest = String::from_utf8_lossy(&std::mem::take(&mut self.buffer)).into_owned();
            self.line(rest.trim_end_matches(['\n', '\r']), &mut out);
        }
        self.dispatch(&mut out);
        out
    }

    fn dispatch(&mut self, out: &mut Vec<String>) {
        if !self.pending.is_empty() {
            out.push(self.pending.join("\n"));
            self.pending.clear();
        }
    }

    fn line(&mut self, line: &str, out: &mut Vec<String>) {
        if line.is_empty() {
            self.dispatch(out);
            return;
        }
        if line.starts_with(':') {
            return; // comment / keep-alive
        }
        if let Some(data) = line.strip_prefix("data:") {
            let data = data.strip_prefix(' ').unwrap_or(data);
            // Some servers omit the blank line between events; a complete JSON
            // object (or [DONE]) already pending is its own event.
            if let Some(previous) = self.pending.last() {
                if previous == "[DONE]" || serde_json::from_str::<Value>(&self.pending.join("\n")).is_ok() {
                    self.dispatch(out);
                }
            }
            self.pending.push(data.to_string());
        }
        // `event:`, `id:` and `retry:` fields are not needed.
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Default)]
pub struct Usage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Chunk {
    Text(String),
    Reasoning(String),
    Usage(Usage),
    Finish(String),
    Error(String),
    Done,
}

pub fn usage_of(value: &Value) -> Option<Usage> {
    let usage = value.get("usage")?;
    if usage.is_null() {
        return None;
    }
    Some(Usage {
        prompt_tokens: usage.get("prompt_tokens").and_then(Value::as_u64).unwrap_or(0),
        completion_tokens: usage.get("completion_tokens").and_then(Value::as_u64).unwrap_or(0),
    })
}

/// Interprets one streamed payload (`choices[].delta`, usage, errors, [DONE]).
pub fn interpret(payload: &str) -> Vec<Chunk> {
    let payload = payload.trim();
    if payload == "[DONE]" {
        return vec![Chunk::Done];
    }
    let Ok(value) = serde_json::from_str::<Value>(payload) else {
        return vec![];
    };
    let mut chunks = Vec::new();
    if let Some(error) = value.get("error").filter(|e| !e.is_null()) {
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| error.to_string());
        chunks.push(Chunk::Error(message));
        return chunks;
    }
    if let Some(choice) = value.get("choices").and_then(|c| c.get(0)) {
        let delta = choice.get("delta").or_else(|| choice.get("message"));
        if let Some(delta) = delta {
            for key in ["reasoning_content", "reasoning"] {
                if let Some(text) = delta.get(key).and_then(Value::as_str).filter(|t| !t.is_empty()) {
                    chunks.push(Chunk::Reasoning(text.to_string()));
                }
            }
            if let Some(text) = delta.get("content").and_then(Value::as_str).filter(|t| !t.is_empty()) {
                chunks.push(Chunk::Text(text.to_string()));
            }
        }
        if let Some(reason) = choice.get("finish_reason").and_then(Value::as_str) {
            chunks.push(Chunk::Finish(reason.to_string()));
        }
    }
    if let Some(usage) = usage_of(&value) {
        chunks.push(Chunk::Usage(usage));
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_events_split_across_chunks() {
        let mut decoder = SseDecoder::default();
        let mut payloads = decoder.push(b"data: {\"choices\":[{\"delta\":{\"content\":\"\xe5\xaf");
        assert!(payloads.is_empty());
        payloads.extend(decoder.push(b"\xbc\"}}]}\r\n\r\n: keep-alive\n\ndata: [DONE]\n\n"));
        assert_eq!(payloads.len(), 2);
        assert_eq!(interpret(&payloads[0]), vec![Chunk::Text("导".into())]);
        assert_eq!(interpret(&payloads[1]), vec![Chunk::Done]);
    }

    #[test]
    fn tolerates_missing_blank_lines_and_trailing_data() {
        let mut decoder = SseDecoder::default();
        let mut payloads = decoder.push(b"data:{\"choices\":[{\"delta\":{\"content\":\"a\"}}]}\ndata: {\"choices\":[{\"delta\":{\"content\":\"b\"}}]}\n");
        payloads.extend(decoder.finish());
        let texts: Vec<_> = payloads.iter().flat_map(|p| interpret(p)).collect();
        assert_eq!(texts, vec![Chunk::Text("a".into()), Chunk::Text("b".into())]);
    }

    #[test]
    fn reads_reasoning_finish_usage_and_errors() {
        let chunks = interpret(r#"{"choices":[{"delta":{"reasoning_content":"思考"},"finish_reason":null}]}"#);
        assert_eq!(chunks, vec![Chunk::Reasoning("思考".into())]);
        let chunks = interpret(r#"{"choices":[{"delta":{},"finish_reason":"length"}],"usage":{"prompt_tokens":10,"completion_tokens":5}}"#);
        assert_eq!(chunks, vec![Chunk::Finish("length".into()), Chunk::Usage(Usage { prompt_tokens: 10, completion_tokens: 5 })]);
        assert_eq!(interpret(r#"{"error":{"message":"overloaded"}}"#), vec![Chunk::Error("overloaded".into())]);
        assert!(interpret("not json").is_empty());
    }
}
