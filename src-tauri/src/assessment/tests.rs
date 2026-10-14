use super::*;
use crate::security::MemoryStore;
use crate::test_support::{self, MockReply, MockServer};
use std::sync::Arc;

const QUESTIONS: &str = r#"{"questions": [
  {"type": "single_choice", "point": "K{P1}", "question": "导数的几何意义是什么？", "options": ["切线斜率", "曲线长度", "面积", "体积"], "answer": "A", "explanation": "教材第 3 页", "sources": ["S1"]},
  {"type": "true_false", "point": "K{P1}", "question": "导数描述函数的变化率。", "answer": true, "explanation": "定义", "sources": ["S1"]},
  {"type": "short_answer", "point": "K{P1}", "question": "简述导数的几何意义。", "answer": "函数图像在该点切线的斜率", "rubric": ["切线", "斜率"], "sources": ["S1"]}
]}"#;

struct Setup {
    paths: AppPaths,
    ai: AiState,
    chapter: i64,
    point: i64,
}

fn setup(server: &MockServer) -> Setup {
    let (paths, mut conn) = test_support::course_with_document();
    let ai = AiState::new(Arc::new(MemoryStore::default()));
    profiles::save(&mut conn, ai.secrets.as_ref(), profiles::ProfileInput {
        id: None, name: "Mock".into(), base_url: server.url(), model: "mock-quiz".into(), temperature: 0.2, top_p: None,
        max_output_tokens: 1024, context_window: 16000, timeout_secs: 10, json_mode: true, api_key: None,
    })
    .unwrap();
    let chapter: i64 = conn.query_row("SELECT id FROM document_sections WHERE title = '第二章 导数'", [], |r| r.get(0)).unwrap();
    knowledge::from_headings(&conn, 1, None).unwrap();
    let point = knowledge::in_section(&conn, chapter).unwrap()[0].id;
    Setup { paths, ai, chapter, point }
}

fn questions_for(point: i64) -> String {
    QUESTIONS.replace("{P1}", &point.to_string())
}

fn answer(conn: &Connection, id: i64, response: Option<&str>, skipped: bool) -> AppResult<Question> {
    save_answer(conn, AnswerInput { question_id: id, response: response.map(String::from), skipped })
}

#[tokio::test]
async fn a_chapter_quiz_is_generated_answered_graded_and_scheduled() {
    let server = MockServer::start(vec![MockReply::Json(String::new())]).await;
    let s = setup(&server);
    let server = MockServer::start(vec![
        MockReply::Json(questions_for(s.point)),
        MockReply::Json(r#"{"results": [{"id": "Q3", "score": 0.5, "feedback": "提到了斜率 [S1]", "missing": ["切线"]}]}"#.into()),
    ])
    .await;
    {
        let conn = s.paths.connect().unwrap();
        conn.execute("UPDATE model_profiles SET base_url = ?1", [server.url()]).unwrap();
    }
    let created = create(&s.paths, &s.ai, CreateQuiz { course_id: 1, section_id: Some(s.chapter), origin: Some("chapter".into()), ..Default::default() }).await.unwrap();
    assert_eq!(created.quiz.title, "第二章 导数 · 小测");
    assert_eq!(created.quiz.question_count, 3);
    // While answering, answers and explanations stay hidden.
    assert!(created.questions.iter().all(|q| q.answer.is_none() && q.explanation.is_none()));
    assert!(created.sources.is_empty());
    let request: serde_json::Value = serde_json::from_str(&server.last_request().body).unwrap();
    let prompt = request["messages"][1]["content"].as_str().unwrap();
    assert!(prompt.contains(&format!("[K{}] 第二章 导数", s.point)) && prompt.contains("<<<资料 S1 | 高等数学讲义.pdf · 第二章 导数 · 第 3 页>>>"));
    assert_eq!(request["response_format"]["type"], "json_object");

    let conn = s.paths.connect().unwrap();
    let ids: Vec<i64> = created.questions.iter().map(|q| q.id).collect();
    assert!(answer(&conn, ids[0], Some("E"), false).is_err());
    answer(&conn, ids[0], Some("a"), false).unwrap();
    answer(&conn, ids[1], Some("false"), false).unwrap();
    answer(&conn, ids[2], Some("就是斜率"), false).unwrap();
    assert_eq!(get(&conn, created.quiz.id).unwrap().current_index, 2);
    drop(conn);

    let graded = submit(&s.paths, &s.ai, created.quiz.id).await.unwrap();
    assert_eq!(graded.quiz.status, "graded");
    let scores: Vec<Option<f64>> = graded.questions.iter().map(|q| q.score).collect();
    assert_eq!(scores, vec![Some(1.0), Some(0.0), Some(0.5)]);
    assert_eq!(graded.quiz.score, Some(50.0));
    assert!(graded.questions[2].feedback.contains("提到了斜率") && graded.questions[2].grading == "ai");
    assert_eq!(graded.questions[0].answer.as_deref(), Some("A"));
    assert_eq!(graded.sources.len(), 1);
    // The grader saw the reference answer, the fenced student answer and the material.
    let request: serde_json::Value = serde_json::from_str(&server.last_request().body).unwrap();
    let grading = request["messages"][1]["content"].as_str().unwrap();
    assert!(grading.contains("<<<学生答案 Q3>>>\n就是斜率") && grading.contains("参考答案：函数图像在该点切线的斜率") && grading.contains("<<<资料 S1 |"));

    let conn = s.paths.connect().unwrap();
    // Mastery: one quiz at 50% → learned, weak, review tomorrow.
    let outcome = &graded.outcome[0];
    assert_eq!((outcome.knowledge_point_id, outcome.result, outcome.status.as_str()), (s.point, 0.5, "learned"));
    assert_eq!(outcome.due_date.as_deref(), Some((mastery::today() + chrono::Duration::days(1)).format("%Y-%m-%d").to_string().as_str()));
    // Wrong and half-right answers land in the mistake book.
    let mut mistaken: Vec<i64> = list_mistakes(&conn, 1, false).unwrap().iter().map(|m| m.question_id).collect();
    mistaken.sort_unstable();
    assert_eq!(mistaken, vec![ids[1], ids[2]]);
    assert_eq!(study::state(&conn, 1).unwrap().activity.last().unwrap().events, 1);
    // Submitting again changes nothing (mastery applied once).
    drop(conn);
    let again = submit(&s.paths, &s.ai, created.quiz.id).await.unwrap();
    assert_eq!(again.quiz.score, Some(50.0));
    let conn = s.paths.connect().unwrap();
    assert_eq!(mastery::get(&conn, s.point).unwrap().unwrap().attempts, 3);
    drop(conn);
    test_support::cleanup(s.paths);
}

#[tokio::test]
async fn failed_ai_grading_falls_back_to_self_grading() {
    let server = MockServer::start(vec![MockReply::Json(String::new())]).await;
    let s = setup(&server);
    let server = MockServer::start(vec![MockReply::Json(questions_for(s.point)), MockReply::Status(401, r#"{"error":{"message":"bad key"}}"#.into())]).await;
    s.paths.connect().unwrap().execute("UPDATE model_profiles SET base_url = ?1", [server.url()]).unwrap();
    let quiz = create(&s.paths, &s.ai, CreateQuiz { course_id: 1, knowledge_point_ids: vec![s.point], allow_skip: Some(true), ..Default::default() }).await.unwrap();
    let conn = s.paths.connect().unwrap();
    answer(&conn, quiz.questions[0].id, Some("A"), false).unwrap();
    answer(&conn, quiz.questions[1].id, None, true).unwrap();
    answer(&conn, quiz.questions[2].id, Some("切线的斜率"), false).unwrap();
    drop(conn);
    let submitted = submit(&s.paths, &s.ai, quiz.quiz.id).await.unwrap();
    assert_eq!((submitted.quiz.status.as_str(), submitted.quiz.pending), ("grading", 1));
    assert!(submitted.quiz.grading_error.as_deref().unwrap().contains("AI 批改失败"));
    assert_eq!(submitted.questions[1].feedback, "未作答（跳过）");
    let conn = s.paths.connect().unwrap();
    assert!(mastery::get(&conn, s.point).unwrap().is_none());
    assert!(self_grade(&conn, quiz.questions[0].id, 1.0).is_err());
    let done = self_grade(&conn, quiz.questions[2].id, 1.0).unwrap();
    assert_eq!((done.quiz.status.as_str(), done.quiz.score), ("graded", Some(66.7)));
    assert_eq!(done.questions[2].grading, "self");
    assert!(self_grade(&conn, quiz.questions[2].id, 0.0).is_err());
    assert_eq!(mastery::get(&conn, s.point).unwrap().unwrap().attempts, 3);
    drop(conn);
    test_support::cleanup(s.paths);
}

#[tokio::test]
async fn strict_quizzes_and_learning_steps() {
    let server = MockServer::start(vec![MockReply::Json(String::new())]).await;
    let s = setup(&server);
    let server = MockServer::start(vec![MockReply::Json(questions_for(s.point))]).await;
    let mut conn = s.paths.connect().unwrap();
    conn.execute("UPDATE model_profiles SET base_url = ?1", [server.url()]).unwrap();
    let path = paths::from_template(&mut conn, paths::TemplateInput { course_id: 1, name: None, mode: None, document_ids: vec![], include_quiz: true }).unwrap();
    let step = path.steps.iter().find(|st| st.kind == "quiz" && st.section_id == Some(s.chapter)).unwrap().clone();
    drop(conn);
    let input = || CreateQuiz { course_id: 1, step_id: Some(step.id), allow_skip: Some(false), kinds: Some(vec!["single_choice".into(), "true_false".into()]), ..Default::default() };
    let quiz = create(&s.paths, &s.ai, input()).await.unwrap();
    assert_eq!((quiz.quiz.origin.as_str(), quiz.quiz.section_id, quiz.quiz.question_count), ("step", Some(s.chapter), 2));
    // Opening the step again re-opens the same unfinished quiz instead of writing a new one.
    assert_eq!(create(&s.paths, &s.ai, input()).await.unwrap().quiz.id, quiz.quiz.id);
    assert_eq!(server.request_count(), 1);
    let conn = s.paths.connect().unwrap();
    assert!(answer(&conn, quiz.questions[0].id, None, true).is_err());
    answer(&conn, quiz.questions[0].id, Some("A"), false).unwrap();
    drop(conn);
    assert!(submit(&s.paths, &s.ai, quiz.quiz.id).await.unwrap_err().to_string().contains("还有 1 道题没有作答"));
    let conn = s.paths.connect().unwrap();
    answer(&conn, quiz.questions[1].id, Some("true"), false).unwrap();
    drop(conn);
    let graded = submit(&s.paths, &s.ai, quiz.quiz.id).await.unwrap();
    assert_eq!(graded.quiz.score, Some(100.0));
    let conn = s.paths.connect().unwrap();
    let step = paths::get_step(&conn, step.id).unwrap();
    assert_eq!((step.status.as_str(), step.note.as_str()), ("done", "小测得分 100%"));
    assert!(answer(&conn, quiz.questions[1].id, Some("false"), false).is_err());
    drop(conn);
    test_support::cleanup(s.paths);
}

#[tokio::test]
async fn mistakes_can_be_redone_and_resolved() {
    let server = MockServer::start(vec![MockReply::Json(String::new())]).await;
    let s = setup(&server);
    let tf_only = |answer: bool| format!(r#"{{"questions": [{{"type": "true_false", "point": "K{}", "question": "导数描述变化率（{answer}）", "answer": {answer}}}]}}"#, s.point);
    let server = MockServer::start(vec![MockReply::Json(tf_only(true)), MockReply::Json(tf_only(false))]).await;
    s.paths.connect().unwrap().execute("UPDATE model_profiles SET base_url = ?1", [server.url()]).unwrap();
    let first = create(&s.paths, &s.ai, CreateQuiz { course_id: 1, knowledge_point_ids: vec![s.point], ..Default::default() }).await.unwrap();
    answer(&s.paths.connect().unwrap(), first.questions[0].id, Some("false"), false).unwrap();
    submit(&s.paths, &s.ai, first.quiz.id).await.unwrap();
    assert_eq!(list_mistakes(&s.paths.connect().unwrap(), 1, false).unwrap().len(), 1);

    let redo = create(&s.paths, &s.ai, CreateQuiz { course_id: 1, origin: Some("mistakes".into()), ..Default::default() }).await.unwrap();
    assert!(redo.quiz.title.starts_with("错题重练"));
    let request: serde_json::Value = serde_json::from_str(&server.last_request().body).unwrap();
    assert!(request["messages"][1]["content"].as_str().unwrap().contains("【学生曾经答错的题】"));
    answer(&s.paths.connect().unwrap(), redo.questions[0].id, Some("false"), false).unwrap();
    submit(&s.paths, &s.ai, redo.quiz.id).await.unwrap();
    let conn = s.paths.connect().unwrap();
    assert!(list_mistakes(&conn, 1, false).unwrap().is_empty());
    let all = list_mistakes(&conn, 1, true).unwrap();
    assert!(all.len() == 1 && all[0].resolved);
    resolve_mistake(&conn, all[0].id, false).unwrap();
    assert_eq!(list_mistakes(&conn, 1, false).unwrap().len(), 1);
    assert!(prepare(&conn, &CreateQuiz { course_id: 1, ..Default::default() }).is_err());
    assert!(prepare(&conn, &CreateQuiz { course_id: 2, origin: Some("review".into()), ..Default::default() }).is_err());
    drop(conn);
    test_support::cleanup(s.paths);
}

#[test]
fn chapters_without_points_get_one_before_a_quiz() {
    let (paths, conn) = test_support::course_with_document();
    let chapter: i64 = conn.query_row("SELECT id FROM document_sections WHERE title = '第二章 导数'", [], |r| r.get(0)).unwrap();
    let prepared = prepare(&conn, &CreateQuiz { course_id: 1, section_id: Some(chapter), question_count: Some(99), ..Default::default() }).unwrap();
    assert_eq!(prepared.points.iter().map(|p| p.title.as_str()).collect::<Vec<_>>(), vec!["第二章 导数"]);
    assert_eq!(prepared.count, 20);
    assert_eq!(prepared.kinds.len(), 5);
    let (text, sources, truncated) = material(&conn, 1, &prepared.points, 10_000).unwrap();
    assert!(text.contains("导数描述函数在某一点的变化率") && sources.len() == 1 && !truncated);
    assert!(prepare(&conn, &CreateQuiz { course_id: 2, section_id: Some(chapter), ..Default::default() }).is_err());
    drop(conn);
    test_support::cleanup(paths);
}
