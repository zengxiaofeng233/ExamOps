//! Helpers shared by unit tests: temporary data directories, sample courses and
//! a scripted OpenAI-compatible server.
#![allow(dead_code)]

use crate::database::AppPaths;
use crate::documents::{self, parse::{Extraction, ParsedBlock, ParsedPage}};
use rusqlite::Connection;

pub fn temp_paths(tag: &str) -> AppPaths {
    let folder = std::env::temp_dir().join(format!(
        "examops-{tag}-{}-{}",
        std::process::id(),
        chrono::Utc::now().timestamp_nanos_opt().unwrap()
    ));
    let paths = AppPaths::new(folder);
    crate::database::init_database(&paths).unwrap();
    paths
}

pub fn cleanup(paths: AppPaths) {
    let _ = std::fs::remove_dir_all(paths.data_dir);
}

fn block(text: &str, heading: Option<u8>) -> ParsedBlock {
    ParsedBlock { text: text.into(), heading, kind: "text".into(), ..Default::default() }
}

/// A three-page calculus excerpt with two chapters and one sub-section.
pub fn sample_extraction() -> Extraction {
    let page = |n: i64, blocks| ParsedPage {
        number: n,
        label: format!("第 {n} 页"),
        width: Some(595.0),
        height: Some(842.0),
        ocr: false,
        loc_start: n,
        loc_end: n,
        blocks,
    };
    Extraction {
        content: "第一章 函数与极限\n函数的定义……".into(),
        loc_kind: "page".into(),
        structure_source: "patterns".into(),
        pages: vec![
            page(1, vec![block("第一章 函数与极限", Some(1)), block("函数是两个非空数集之间的对应关系。", None)]),
            page(2, vec![block("1.1 极限的定义", Some(2)), block("数列极限描述了数列的变化趋势，导数建立在极限之上。", None)]),
            page(3, vec![block("第二章 导数", Some(1)), block("导数描述函数在某一点的变化率，导数的几何意义是切线斜率。", None)]),
        ],
        images: vec![],
        warnings: vec![],
    }
}

/// Course 1 with an indexed sample document (id 1); course 2 is empty.
pub fn course_with_document() -> (AppPaths, Connection) {
    let paths = temp_paths("course");
    let mut conn = paths.connect().unwrap();
    conn.execute("INSERT INTO courses (name) VALUES ('高等数学'), ('线性代数')", []).unwrap();
    let source = paths.data_dir.join("高等数学讲义.pdf");
    std::fs::write(&source, "%PDF-1.4").unwrap();
    let document = documents::prepare(&mut conn, &paths.documents_dir, 1, &source).unwrap();
    documents::apply_extraction(&mut conn, document.id, &sample_extraction()).unwrap();
    (paths, conn)
}

// --------------------------------------------------------------------------- mock AI server

/// One scripted reply of the mock OpenAI-compatible server.
#[derive(Clone, Debug)]
pub enum MockReply {
    /// SSE deltas followed by `[DONE]`.
    Stream(Vec<&'static str>),
    /// SSE deltas, then the connection is dropped mid-response.
    StreamThenDrop(Vec<&'static str>),
    /// SSE deltas with a pause (ms) before each one.
    Slow(Vec<&'static str>, u64),
    /// A non-streamed JSON completion with this content.
    Json(String),
    /// An HTTP error.
    Status(u16, String),
}

#[derive(Clone, Debug, Default)]
pub struct RecordedRequest {
    pub headers: String,
    pub body: String,
}

pub struct MockServer {
    port: u16,
    requests: std::sync::Arc<std::sync::Mutex<Vec<RecordedRequest>>>,
}

impl MockServer {
    /// Serves `replies` in order (the last one repeats).
    pub async fn start(replies: Vec<MockReply>) -> MockServer {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let requests = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let log = requests.clone();
        tokio::spawn(async move {
            let mut index = 0usize;
            loop {
                let Ok((mut socket, _)) = listener.accept().await else { break };
                let reply = replies[index.min(replies.len() - 1)].clone();
                index += 1;
                let log = log.clone();
                tokio::spawn(async move {
                    // Read headers and a Content-Length body.
                    let mut data = Vec::new();
                    let mut buffer = [0u8; 8192];
                    let header_end = loop {
                        let n = socket.read(&mut buffer).await.unwrap_or(0);
                        if n == 0 {
                            return;
                        }
                        data.extend_from_slice(&buffer[..n]);
                        if let Some(pos) = data.windows(4).position(|w| w == b"\r\n\r\n") {
                            break pos + 4;
                        }
                    };
                    let headers = String::from_utf8_lossy(&data[..header_end]).to_lowercase();
                    let length = headers
                        .lines()
                        .find_map(|l| l.strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0)))
                        .unwrap_or(0);
                    while data.len() < header_end + length {
                        let n = socket.read(&mut buffer).await.unwrap_or(0);
                        if n == 0 {
                            break;
                        }
                        data.extend_from_slice(&buffer[..n]);
                    }
                    let body = String::from_utf8_lossy(&data[header_end..]).into_owned();
                    log.lock().unwrap().push(RecordedRequest { headers, body });

                    let delta = |text: &str| {
                        format!("data: {}\n\n", serde_json::json!({"choices": [{"delta": {"content": text}, "finish_reason": null}]}))
                    };
                    let chunk = |payload: &str| format!("{:x}\r\n{payload}\r\n", payload.len());
                    let sse_head = "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ntransfer-encoding: chunked\r\nconnection: close\r\n\r\n";
                    match reply {
                        MockReply::Stream(parts) => {
                            let _ = socket.write_all(sse_head.as_bytes()).await;
                            for part in parts {
                                let _ = socket.write_all(chunk(&delta(part)).as_bytes()).await;
                            }
                            let _ = socket.write_all(chunk("data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n").as_bytes()).await;
                            let _ = socket.write_all(b"0\r\n\r\n").await;
                        }
                        MockReply::Slow(parts, pause) => {
                            let _ = socket.write_all(sse_head.as_bytes()).await;
                            for part in parts {
                                tokio::time::sleep(std::time::Duration::from_millis(pause)).await;
                                if socket.write_all(chunk(&delta(part)).as_bytes()).await.is_err() {
                                    return;
                                }
                            }
                            let _ = socket.write_all(chunk("data: [DONE]\n\n").as_bytes()).await;
                            let _ = socket.write_all(b"0\r\n\r\n").await;
                        }
                        MockReply::StreamThenDrop(parts) => {
                            let _ = socket.write_all(sse_head.as_bytes()).await;
                            for part in parts {
                                let _ = socket.write_all(chunk(&delta(part)).as_bytes()).await;
                            }
                            let _ = socket.flush().await;
                            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                            // Drop without the terminating chunk: the client sees an aborted body.
                        }
                        MockReply::Json(content) => {
                            let body = serde_json::json!({
                                "choices": [{"message": {"role": "assistant", "content": content}, "finish_reason": "stop"}],
                                "usage": {"prompt_tokens": 11, "completion_tokens": 7}
                            })
                            .to_string();
                            let _ = socket
                                .write_all(format!("HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len()).as_bytes())
                                .await;
                        }
                        MockReply::Status(status, body) => {
                            let _ = socket
                                .write_all(format!("HTTP/1.1 {status} Error\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len()).as_bytes())
                                .await;
                        }
                    }
                    let _ = socket.shutdown().await;
                });
            }
        });
        MockServer { port, requests }
    }

    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/v1", self.port)
    }

    pub fn last_request(&self) -> RecordedRequest {
        self.requests.lock().unwrap().last().cloned().unwrap_or_default()
    }

    pub fn request_count(&self) -> usize {
        self.requests.lock().unwrap().len()
    }
}
