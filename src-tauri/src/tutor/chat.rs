//! One question → one streamed, cited answer (Stage 3 pipeline; Stage 4 adds
//! teaching plans on top of it).

use super::{images::{self, ImageAttachment, IMAGE_TOKEN_RESERVE}, prompts, Message};
use crate::ai::{
    self,
    client::{ChatMessage, ChatOptions, StreamEvent},
    error::GatewayError,
    profiles, tokens, AiState, InflightGuard,
};
use crate::context::{self, BuiltContext, ContextRequest, ConversationScope};
use crate::database::AppPaths;
use crate::error::{AppError, AppResult};
use crate::settings;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SendInput {
    pub conversation_id: i64,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub images: Vec<ImageAttachment>,
    /// Re-answer the last question instead of asking a new one.
    #[serde(default)]
    pub regenerate: bool,
    /// Teaching action (`start`, `continue`, `example`…); `None` = a question.
    #[serde(default)]
    pub action: Option<String>,
    /// Lesson part for the `goto` action.
    #[serde(default)]
    pub target: Option<usize>,
}

/// Runs after a complete answer (e.g. advances the teaching progress).
pub type AfterHook = Box<dyn FnOnce(&Connection, &Message) -> AppResult<()> + Send>;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)] // short-lived events sent straight to the UI
pub enum ChatEvent {
    Started { request_id: String, user_message: Option<Message>, assistant_message: Message },
    Delta { text: String },
    Reasoning { text: String },
    Done { message: Message },
}

/// What to ask the model this turn; teaching modes build richer plans.
pub struct TurnPlan {
    /// Text stored as the student's message (shown in the chat).
    pub display: String,
    /// Text sent to the model as the student's message.
    pub prompt: String,
    pub task: String,
    pub retrieval_query: String,
    pub scope: ConversationScope,
    /// Extra fenced material that must be included (e.g. the slice being taught).
    pub primary: Option<BuiltContext>,
    /// Whether the Context Engine adds material from `scope` (Full/RAG/Auto).
    pub retrieve: bool,
    pub meta: Value,
    pub after: Option<AfterHook>,
}

pub const MAX_QUESTION_CHARS: usize = 8000;

/// Previous turns, oldest first, within `max_tokens`.
pub fn history(conn: &Connection, conversation_id: i64, before_id: i64, turns: usize, max_tokens: usize) -> AppResult<Vec<ChatMessage>> {
    if turns == 0 {
        return Ok(Vec::new());
    }
    let mut stmt = conn.prepare(
        "SELECT role, content, meta FROM messages WHERE conversation_id = ?1 AND id < ?2 AND content != ''
         AND status IN ('done', 'cancelled') ORDER BY id DESC LIMIT ?3",
    )?;
    let rows = stmt.query_map(params![conversation_id, before_id, (turns * 2) as i64], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))?;
    let mut picked = Vec::new();
    let mut used = 0;
    for row in rows {
        let (role, content, meta) = row?;
        let images = images::from_meta(&serde_json::from_str(&meta)?)?.into_iter().map(|image| image.data_url).collect::<Vec<_>>();
        let cost = tokens::estimate(&content) + tokens::MESSAGE_OVERHEAD + images.len() * IMAGE_TOKEN_RESERVE;
        if used + cost > max_tokens {
            break;
        }
        used += cost;
        picked.push(ChatMessage { role, content, images });
    }
    picked.reverse();
    // Start with a question so the exchange reads naturally.
    while picked.first().map(|m| m.role != "user").unwrap_or(false) {
        picked.remove(0);
    }
    Ok(picked)
}

/// Budget left for course material after rules, history, question and answer.
pub fn material_budget(profile: &profiles::ResolvedProfile, fixed: &[&str], history: &[ChatMessage]) -> usize {
    let used: usize = fixed.iter().map(|t| tokens::estimate(t) + tokens::MESSAGE_OVERHEAD).sum::<usize>()
        + history.iter().map(|m| tokens::estimate(&m.content) + tokens::MESSAGE_OVERHEAD + m.images.len() * IMAGE_TOKEN_RESERVE).sum::<usize>();
    profile
        .context_window
        .saturating_sub(profile.max_output_tokens as usize)
        .saturating_sub(used)
        .saturating_sub(300)
}

/// Runs one turn: stores the question, builds context, streams and stores the answer.
pub async fn run_turn(
    paths: &AppPaths,
    ai: &AiState,
    input: &SendInput,
    plan_for: impl FnOnce(&Connection, &super::Conversation, &str) -> AppResult<TurnPlan>,
    emit: impl Fn(ChatEvent) + Send + Sync,
) -> AppResult<Message> {
    let conversation_id = input.conversation_id;
    let regenerate = input.regenerate;
    images::validate(&input.images)?;
    let conn = paths.connect()?;
    let conversation = super::get(&conn, conversation_id)?;
    let course = crate::commands::course::find(&conn, conversation.course_id)?;
    let app_settings = settings::load(&conn)?;
    let profile = profiles::resolve(&conn, ai.secrets.as_ref(), conversation.profile_id)?;

    // Regenerate re-asks the last question; otherwise store a new one.
    let (question_text, history_before, images) = if regenerate {
        let last = super::last_user_message(&conn, conversation_id)?.ok_or_else(|| AppError::invalid("还没有可以重新生成的问题"))?;
        super::delete_after(&conn, conversation_id, last.id)?;
        let original = last.meta.get("input").and_then(Value::as_str).unwrap_or(&last.content).to_string();
        (original, last.id, images::from_meta(&last.meta)?)
    } else {
        let text = input.content.trim();
        if text.chars().count() > MAX_QUESTION_CHARS {
            return Err(AppError::invalid(format!("问题过长（最多 {MAX_QUESTION_CHARS} 字）")));
        }
        let text = if text.is_empty() && !input.images.is_empty() { "请识别图片中的题目或手写答案，并结合当前学习内容讲解或点评。" } else { text };
        (text.to_string(), i64::MAX, input.images.clone())
    };
    let plan = plan_for(&conn, &conversation, &question_text)?;
    if plan.prompt.trim().is_empty() {
        return Err(AppError::invalid("请输入问题"));
    }
    let user_message = if !regenerate {
        Some(super::insert_message(
            &conn,
            conversation_id,
            "user",
            &plan.display,
            "done",
            &[],
            &json!({ "input": question_text, "plan": plan.meta, "images": images }),
        )?)
    } else {
        None
    };
    let before = user_message.as_ref().map(|m| m.id).unwrap_or(history_before);
    let history = history(&conn, conversation_id, before, app_settings.context.history_turns, profile.context_window / 4)?;

    let task = if !images.is_empty() || history.iter().any(|message| !message.images.is_empty()) {
        format!("{}\n{}", plan.task, prompts::IMAGE_RULES)
    } else { plan.task };
    let fixed_prompt = prompts::system_prompt(&course.name, &task, &BuiltContext::default());
    let primary_tokens = plan.primary.as_ref().map(|p| p.tokens).unwrap_or(0);
    let budget = material_budget(&profile, &[&fixed_prompt, &plan.prompt], &history).saturating_sub(primary_tokens + images.len() * IMAGE_TOKEN_RESERVE);
    let mut built = if plan.retrieve {
        context::build(&conn, &ContextRequest {
            course_id: conversation.course_id,
            query: &plan.retrieval_query,
            mode: conversation.context_mode,
            scope: &plan.scope,
            budget_tokens: budget,
            top_k: app_settings.context.rag_top_k,
            full_ratio: app_settings.context.full_ratio,
        })?
    } else {
        BuiltContext::default()
    };
    if let Some(primary) = plan.primary {
        built = merge_primary(primary, built);
    }
    let after = plan.after;
    let system = prompts::system_prompt(&course.name, &task, &built);
    let mut messages = vec![ChatMessage::system(system)];
    messages.extend(history);
    messages.push(ChatMessage { role: "user".into(), content: plan.prompt.clone(), images: images.into_iter().map(|image| image.data_url).collect() });
    let prompt_tokens: usize = messages.iter().map(|m| tokens::estimate(&m.content) + tokens::MESSAGE_OVERHEAD + m.images.len() * IMAGE_TOKEN_RESERVE).sum();

    let mut meta = json!({
        "profile": profile.name,
        "model": profile.model,
        "context_mode": built.mode_used,
        "context_tokens": built.tokens,
        "material_tokens": built.material_tokens,
        "context_truncated": built.truncated,
        "notice": built.notice,
        "prompt_tokens_estimate": prompt_tokens,
        "plan": plan.meta,
    });
    let assistant = super::insert_message(&conn, conversation_id, "assistant", "", "streaming", &built.sources, &meta)?;
    let request_id = format!("msg-{}", assistant.id);
    emit(ChatEvent::Started { request_id: request_id.clone(), user_message, assistant_message: assistant.clone() });

    let (_guard, cancel) = InflightGuard::new(ai, &request_id);
    let mut text = String::new();
    let mut last_save = Instant::now();
    let outcome = ai
        .gateway
        .stream(&profile, &messages, &ChatOptions::default(), &cancel, |event| match event {
            StreamEvent::Text(delta) => {
                text.push_str(delta);
                emit(ChatEvent::Delta { text: delta.to_string() });
                if last_save.elapsed() > Duration::from_millis(800) {
                    // A short-lived connection keeps the streaming future `Send`.
                    if let Ok(partial) = paths.connect() {
                        let _ = super::save_partial(&partial, assistant.id, &text);
                    }
                    last_save = Instant::now();
                }
            }
            StreamEvent::Reasoning(delta) => emit(ChatEvent::Reasoning { text: delta.to_string() }),
        })
        .await;
    let completion = outcome.completion;
    let (status, error) = match &outcome.error {
        None => ("done", None),
        Some(GatewayError::Cancelled) => ("cancelled", None),
        Some(other) => {
            let hint = if messages.iter().any(|message| !message.images.is_empty()) {
                "\n本次请求包含图片，请确认当前模型及接口支持图片输入。"
            } else { "" };
            ("error", Some(format!("{other}{hint}")))
        }
    };
    let sources = context::finalize_sources(built.sources, &completion.text, &built.mode_used, built.pinned);
    meta["elapsed_ms"] = json!(completion.elapsed_ms);
    meta["finish_reason"] = json!(completion.finish_reason);
    meta["output_truncated"] = json!(completion.finish_reason.as_deref() == Some("length"));
    if !completion.reasoning.is_empty() {
        meta["reasoning"] = json!(completion.reasoning.chars().take(6000).collect::<String>());
    }
    if let Some(error) = &outcome.error {
        meta["error_kind"] = json!(error.kind());
        if matches!(error, GatewayError::Interrupted(_) | GatewayError::Timeout) {
            meta["partial"] = json!(true);
        }
    }
    let (prompt_used, completion_used, estimated) = match completion.usage {
        Some(usage) => (usage.prompt_tokens, usage.completion_tokens, false),
        None => (prompt_tokens as u64, tokens::estimate(&completion.text) as u64, true),
    };
    meta["usage"] = json!({ "prompt": prompt_used, "completion": completion_used, "estimated": estimated });
    let message = super::finish_message(&conn, assistant.id, &completion.text, status, error.as_deref(), &sources, &meta)?;
    if status == "done" {
        if let Some(after) = after {
            after(&conn, &message)?;
        }
    }
    let first = super::messages(&conn, conversation_id)?.into_iter().find(|m| m.role == "user").map(|m| m.content);
    super::touch(&conn, conversation_id, first.as_deref())?;
    crate::study::set_conversation(&conn, conversation.course_id, conversation_id, conversation.section_id)?;
    if completion_used > 0 || !estimated {
        ai::record_usage(&conn, profile.id, "chat", prompt_used, completion_used, estimated)?;
    }
    emit(ChatEvent::Done { message: message.clone() });
    Ok(message)
}

/// Puts the mandatory material first (its sources stay pinned in the answer)
/// and renumbers the retrieved sources after it.
pub fn merge_primary(primary: BuiltContext, extra: BuiltContext) -> BuiltContext {
    let pinned = primary.sources.len();
    let known: Vec<i64> = primary.sources.iter().map(|s| s.chunk_id).collect();
    let mut mapping: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut extra_sources = Vec::new();
    for source in extra.sources.into_iter().filter(|s| !known.contains(&s.chunk_id)) {
        let label = format!("S{}", pinned + extra_sources.len() + 1);
        mapping.insert(source.label.clone(), label.clone());
        extra_sources.push(context::SourceRef { label, ..source });
    }
    if extra_sources.is_empty() {
        return BuiltContext { pinned, ..primary };
    }
    // Relabel fence by fence (a global replace could rename a label twice).
    let mut text = primary.text;
    text.push_str("【相关补充资料】\n");
    for part in extra.text.split("<<<资料 ").filter(|p| !p.trim().is_empty()) {
        let Some((old, rest)) = part.split_once(" |") else { continue };
        let Some(new) = mapping.get(old) else { continue };
        let rest = rest.replace(&format!("<<<{old} 结束>>>"), &format!("<<<{new} 结束>>>"));
        text.push_str(&format!("<<<资料 {new} |{rest}"));
    }
    let mut sources = primary.sources;
    sources.extend(extra_sources);
    BuiltContext {
        mode_used: extra.mode_used,
        tokens: primary.tokens + extra.tokens,
        text,
        sources,
        material_tokens: extra.material_tokens,
        truncated: extra.truncated,
        notice: primary.notice.or(extra.notice),
        pinned,
    }
}

/// Plain question answering over the course (Stage 3).
pub fn qa_plan(_conn: &Connection, conversation: &super::Conversation, question: &str) -> AppResult<TurnPlan> {
    Ok(TurnPlan {
        display: question.to_string(),
        prompt: question.to_string(),
        task: prompts::QA_TASK.to_string(),
        retrieval_query: question.to_string(),
        scope: conversation.scope.clone(),
        primary: None,
        retrieve: true,
        meta: json!({ "action": "ask" }),
        after: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::MemoryStore;
    use crate::test_support::{self, MockReply, MockServer};
    use std::sync::{Arc, Mutex};

    fn setup(server: &MockServer) -> (AppPaths, AiState, i64) {
        let (paths, mut conn) = test_support::course_with_document();
        let ai = AiState::new(Arc::new(MemoryStore::default()));
        let input = profiles::ProfileInput {
            id: None, name: "Mock".into(), base_url: server.url(), model: "mock-model".into(), temperature: 0.2, top_p: None,
            max_output_tokens: 512, context_window: 16000, timeout_secs: 10, json_mode: false, api_key: Some("sk-mock-123456789".into()),
        };
        profiles::save(&mut conn, ai.secrets.as_ref(), input).unwrap();
        let conversation = super::super::create(&conn, super::super::CreateConversation { course_id: 1, mode: None, title: None, context_mode: None, scope: None }, crate::context::ContextMode::Auto).unwrap();
        (paths, ai, conversation.id)
    }

    async fn ask(paths: &AppPaths, ai: &AiState, id: i64, question: &str, events: Arc<Mutex<Vec<ChatEvent>>>) -> AppResult<Message> {
        run_turn(paths, ai, &SendInput { conversation_id: id, content: question.into(), ..Default::default() }, qa_plan, move |e| events.lock().unwrap().push(e)).await
    }

    #[tokio::test]
    async fn streams_a_cited_answer_and_persists_everything() {
        let server = MockServer::start(vec![MockReply::Stream(vec!["【教材】导数是", "函数的变化率 [S3]。"])]).await;
        let (paths, ai, id) = setup(&server);
        let events = Arc::new(Mutex::new(Vec::new()));
        let message = ask(&paths, &ai, id, "什么是导数？", events.clone()).await.unwrap();
        assert_eq!(message.status, "done");
        assert_eq!(message.content, "【教材】导数是函数的变化率 [S3]。");
        // Auto mode sent the whole (small) course; only the cited source is kept.
        assert_eq!(message.meta["context_mode"], "full");
        assert_eq!(message.sources.len(), 1);
        assert!(message.sources[0].cited && message.sources[0].loc_label == "第 3 页");
        let events = events.lock().unwrap();
        assert!(matches!(events.first(), Some(ChatEvent::Started { .. })));
        assert_eq!(events.iter().filter(|e| matches!(e, ChatEvent::Delta { .. })).count(), 2);
        // The request carried the key, the rules, fenced material and the question.
        let request = server.last_request();
        assert!(request.headers.contains("authorization: bearer sk-mock-123456789"));
        let body: Value = serde_json::from_str(&request.body).unwrap();
        let system = body["messages"][0]["content"].as_str().unwrap();
        assert!(system.contains("【教材】") && system.contains("<<<资料 S1 |"));
        assert_eq!(body["messages"].as_array().unwrap().last().unwrap()["content"], "什么是导数？");
        assert_eq!(body["stream"], true);
        let conn = paths.connect().unwrap();
        let detail = super::super::detail(&conn, id).unwrap();
        assert_eq!(detail.messages.len(), 2);
        assert_eq!(detail.conversation.title, "什么是导数？");
        drop(conn);
        test_support::cleanup(paths);
    }

    #[tokio::test]
    async fn follow_up_questions_carry_history_and_regenerate_replaces_answer() {
        let server = MockServer::start(vec![MockReply::Stream(vec!["第一次回答"]), MockReply::Stream(vec!["第二次回答"]), MockReply::Stream(vec!["重新生成"])]).await;
        let (paths, ai, id) = setup(&server);
        let events = Arc::new(Mutex::new(Vec::new()));
        ask(&paths, &ai, id, "导数是什么", events.clone()).await.unwrap();
        ask(&paths, &ai, id, "举个例子", events.clone()).await.unwrap();
        let body: Value = serde_json::from_str(&server.last_request().body).unwrap();
        let roles: Vec<_> = body["messages"].as_array().unwrap().iter().map(|m| m["role"].as_str().unwrap().to_string()).collect();
        assert_eq!(roles, vec!["system", "user", "assistant", "user"]);
        let message = run_turn(&paths, &ai, &SendInput { conversation_id: id, regenerate: true, ..Default::default() }, qa_plan, |_| {}).await.unwrap();
        assert_eq!(message.content, "重新生成");
        let conn = paths.connect().unwrap();
        let contents: Vec<_> = super::super::messages(&conn, id).unwrap().into_iter().map(|m| m.content).collect();
        assert_eq!(contents, vec!["导数是什么", "第一次回答", "举个例子", "重新生成"]);
        drop(conn);
        test_support::cleanup(paths);
    }

    #[tokio::test]
    async fn api_errors_and_interruptions_are_saved_with_partial_text() {
        let server = MockServer::start(vec![
            MockReply::Status(401, r#"{"error":{"message":"invalid api key"}}"#.into()),
            MockReply::StreamThenDrop(vec!["半句"]),
        ])
        .await;
        let (paths, ai, id) = setup(&server);
        let events = Arc::new(Mutex::new(Vec::new()));
        let failed = ask(&paths, &ai, id, "问题一", events.clone()).await.unwrap();
        assert_eq!(failed.status, "error");
        assert!(failed.error.unwrap().contains("API Key 无效"));
        let partial = ask(&paths, &ai, id, "问题二", events.clone()).await.unwrap();
        assert_eq!((partial.status.as_str(), partial.content.as_str()), ("error", "半句"));
        assert!(partial.error.unwrap().contains("连接中断"));
        assert_eq!(partial.meta["partial"], true);
        drop(ai);
        test_support::cleanup(paths);
    }

    #[tokio::test]
    async fn generation_can_be_cancelled() {
        let server = MockServer::start(vec![MockReply::Slow(vec!["一", "二", "三", "四"], 400)]).await;
        let (paths, ai, id) = setup(&server);
        let ai = Arc::new(ai);
        let cancel_ai = ai.clone();
        let events = Arc::new(Mutex::new(Vec::new()));
        let watcher = events.clone();
        let canceller = tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(50)).await;
                let request_id = watcher.lock().unwrap().iter().find_map(|e| match e {
                    ChatEvent::Delta { .. } => Some(()),
                    _ => None,
                });
                if request_id.is_some() {
                    break;
                }
            }
            let id = watcher.lock().unwrap().iter().find_map(|e| match e {
                ChatEvent::Started { request_id, .. } => Some(request_id.clone()),
                _ => None,
            });
            assert!(cancel_ai.cancel(&id.unwrap()));
        });
        let message = ask(&paths, &ai, id, "慢慢回答", events.clone()).await.unwrap();
        canceller.await.unwrap();
        assert_eq!(message.status, "cancelled");
        assert!(!message.content.is_empty() && message.content.chars().count() < 4);
        test_support::cleanup(paths);
    }

    #[test]
    fn merged_material_is_relabelled_fence_by_fence() {
        let chunk = |id: i64, text: &str| crate::search::ChunkHit {
            chunk_id: id, document_id: 1, filename: "讲义.pdf".into(), section_id: None, section_title: None, chunk_index: id,
            page_start: id, page_end: id, loc_label: format!("第 {id} 页"), content: text.into(), score: 0.0,
            unreliable_formula: false,
        };
        let (text, sources) = context::render(&[chunk(10, "甲"), chunk(11, "乙")], 1);
        let primary = BuiltContext { mode_used: "lesson".into(), text, sources, ..Default::default() };
        // Retrieval returned 11 again (duplicate) plus 12 and 13, labelled S1–S3.
        let (text, sources) = context::render(&[chunk(11, "乙"), chunk(12, "丙"), chunk(13, "丁")], 1);
        let extra = BuiltContext { mode_used: "rag".into(), text, sources, ..Default::default() };
        let merged = merge_primary(primary, extra);
        assert_eq!(merged.pinned, 2);
        assert_eq!(merged.sources.iter().map(|s| (s.label.as_str(), s.chunk_id)).collect::<Vec<_>>(), vec![("S1", 10), ("S2", 11), ("S3", 12), ("S4", 13)]);
        assert!(merged.text.contains("<<<资料 S3 | 讲义.pdf · 第 12 页>>>\n丙\n<<<S3 结束>>>"));
        assert!(merged.text.contains("<<<资料 S4 | 讲义.pdf · 第 13 页>>>\n丁\n<<<S4 结束>>>"));
        assert_eq!(merged.text.matches("乙").count(), 1);
        assert_eq!(merged.mode_used, "rag");
        // Nothing new retrieved: the lesson material stands alone.
        let alone = merge_primary(BuiltContext { mode_used: "lesson".into(), sources: merged.sources[..1].to_vec(), ..Default::default() }, BuiltContext::default());
        assert_eq!((alone.mode_used.as_str(), alone.pinned), ("lesson", 1));
    }

    #[tokio::test]
    async fn missing_model_is_reported_before_storing_anything() {
        let (paths, conn) = test_support::course_with_document();
        let ai = AiState::new(Arc::new(MemoryStore::default()));
        let conversation = super::super::create(&conn, super::super::CreateConversation { course_id: 1, mode: None, title: None, context_mode: None, scope: None }, crate::context::ContextMode::Auto).unwrap();
        let error = run_turn(&paths, &ai, &SendInput { conversation_id: conversation.id, content: "问题".into(), ..Default::default() }, qa_plan, |_| {}).await.unwrap_err();
        assert!(error.to_string().contains("尚未配置 AI 模型"));
        assert!(super::super::messages(&conn, conversation.id).unwrap().is_empty());
        drop(conn);
        test_support::cleanup(paths);
    }

    fn picture() -> ImageAttachment {
        ImageAttachment { name: "答案.png".into(), data_url: "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a4qMAAAAASUVORK5CYII=".into() }
    }

    #[tokio::test]
    async fn images_survive_reload_regeneration_and_follow_up() {
        let server = MockServer::start(vec![
            MockReply::Stream(vec!["【图片】这是手写答案"]),
            MockReply::Stream(vec!["重新识别"]),
            MockReply::Stream(vec!["继续解释"]),
        ]).await;
        let (paths, ai, id) = setup(&server);
        let input = SendInput { conversation_id: id, images: vec![picture()], ..Default::default() };
        let message = run_turn(&paths, &ai, &input, qa_plan, |_| {}).await.unwrap();
        assert_eq!(message.status, "done");
        let body: Value = serde_json::from_str(&server.last_request().body).unwrap();
        let content = &body["messages"].as_array().unwrap().last().unwrap()["content"];
        assert!(content[0]["text"].as_str().unwrap().contains("识别图片"));
        assert_eq!(content[1]["image_url"]["url"], picture().data_url);
        assert!(body["messages"][0]["content"].as_str().unwrap().contains("看不清"));
        let conn = paths.connect().unwrap();
        let detail = super::super::detail(&conn, id).unwrap();
        assert_eq!(images::from_meta(&detail.messages[0].meta).unwrap(), vec![picture()]);
        drop(conn);
        run_turn(&paths, &ai, &SendInput { conversation_id: id, regenerate: true, ..Default::default() }, qa_plan, |_| {}).await.unwrap();
        let retry: Value = serde_json::from_str(&server.last_request().body).unwrap();
        assert_eq!(retry["messages"].as_array().unwrap().last().unwrap()["content"], *content);
        ask(&paths, &ai, id, "第二步为什么这么写？", Arc::new(Mutex::new(Vec::new()))).await.unwrap();
        let follow_up: Value = serde_json::from_str(&server.last_request().body).unwrap();
        assert_eq!(follow_up["messages"][1]["content"][1]["image_url"]["url"], picture().data_url);
        assert_eq!(follow_up["messages"].as_array().unwrap().last().unwrap()["content"], "第二步为什么这么写？");
        let conn = paths.connect().unwrap();
        assert_eq!(super::super::messages(&conn, id).unwrap().len(), 4);
        drop(conn);
        test_support::cleanup(paths);
    }

    #[tokio::test]
    async fn all_teaching_modes_accept_image_answers_without_advancing_progress() {
        let server = MockServer::start(vec![MockReply::Stream(vec!["点评答案"]), MockReply::Stream(vec!["点评答案"]), MockReply::Stream(vec!["点评答案"])]).await;
        let (paths, ai, _) = setup(&server);
        for mode in ["learn", "review", "sprint"] {
            let conn = paths.connect().unwrap();
            let section_id = conn.query_row("SELECT id FROM document_sections WHERE title = '第二章 导数' LIMIT 1", [], |row| row.get(0)).unwrap();
            let conversation = super::super::teach::start(&conn, super::super::teach::StartTeaching {
                course_id: 1, mode: mode.into(), section_id: Some(section_id), resource_id: None, step_id: None,
            }, crate::context::ContextMode::Auto).unwrap();
            let progress = conversation.progress;
            drop(conn);
            let message = run_turn(&paths, &ai, &SendInput { conversation_id: conversation.id, content: "这是我的答案".into(), images: vec![picture()], ..Default::default() },
                |conn, conversation, text| super::super::teach::plan(conn, conversation, "ask", None, text), |_| {}).await.unwrap();
            assert_eq!(message.status, "done");
            let body: Value = serde_json::from_str(&server.last_request().body).unwrap();
            assert_eq!(body["messages"].as_array().unwrap().last().unwrap()["content"][1]["image_url"]["url"], picture().data_url);
            let conn = paths.connect().unwrap();
            assert_eq!(super::super::get(&conn, conversation.id).unwrap().progress, progress);
        }
        test_support::cleanup(paths);
    }

    #[tokio::test]
    async fn invalid_images_are_rejected_before_storing_messages() {
        let server = MockServer::start(Vec::new()).await;
        let (paths, ai, id) = setup(&server);
        let invalid = [
            vec![picture(); 5],
            vec![ImageAttachment { name: "remote.png".into(), data_url: "https://example.com/image.png".into() }],
            vec![ImageAttachment { name: "fake.png".into(), data_url: "data:image/png;base64,aGVsbG8=".into() }],
            vec![ImageAttachment { name: "bad.png".into(), data_url: "data:image/png;base64,%%%".into() }],
            vec![ImageAttachment { name: "large.png".into(), data_url: format!("data:image/png;base64,{}", "A".repeat(images::MAX_IMAGE_BYTES * 2)) }],
        ];
        for images in invalid {
            assert!(run_turn(&paths, &ai, &SendInput { conversation_id: id, images, ..Default::default() }, qa_plan, |_| {}).await.is_err());
        }
        let conn = paths.connect().unwrap();
        assert!(super::super::messages(&conn, id).unwrap().is_empty());
        drop(conn);
        test_support::cleanup(paths);
    }
}
