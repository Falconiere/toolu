use serde_json::json;
use tempfile::TempDir;
use toolu_http_test_support::{Fixture, Reply};
use toolu_protocol::exit::Exit;

use super::tests::{conversation_reply_args, on, state};
use crate::recorded;
use crate::verbs::tests::{read, run};

fn post_count(fixture: &Fixture) -> usize {
  fixture
    .requests()
    .unwrap()
    .iter()
    .filter(|request| request.method == "POST")
    .count()
}

#[test]
fn a_reply_recovers_after_its_body_file_is_removed_or_oversized() {
  for oversized in [false, true] {
    let dir = TempDir::new().unwrap();
    let state = state(&dir);
    let body = dir.path().join("body.md");
    std::fs::write(&body, "Thanks, fixed.\n").unwrap();
    let fixture = Fixture::start().unwrap();
    fixture
      .route(
        "/repos/Falconiere/toolu/issues/165/comments",
        Reply::dropped_recorded(),
      )
      .unwrap();
    let list = "/repos/Falconiere/toolu/issues/165/comments?per_page=100";
    fixture.route(list, Reply::new(200, "[]")).unwrap();
    let env = recorded::env(&fixture);
    let args = conversation_reply_args(&state, &body);
    let (exit, first) = run(&args, &on(&fixture, &env));
    assert_eq!(exit, Exit::TempFail, "{first}");
    assert_eq!(first["errors"][0]["code"], "reply_uncertain");
    let posted = read(&state)["actions"]["replyIntents"]["conversation:5"]["body"]
      .as_str()
      .unwrap()
      .to_owned();
    if oversized {
      std::fs::write(&body, "x".repeat(65_537)).unwrap();
    } else {
      std::fs::remove_file(&body).unwrap();
    }
    fixture
      .route(
        list,
        Reply::new(200, json!([{"id": 99, "body": posted}]).to_string()),
      )
      .unwrap();
    let (exit, done) = run(&args, &on(&fixture, &env));
    assert_eq!(exit, Exit::Success, "{done}");
    assert_eq!(done["commentId"], 99);
    assert_eq!(post_count(&fixture), 1);
  }
}

#[test]
fn a_prepared_intent_uses_its_original_body_when_the_file_is_gone() {
  let dir = TempDir::new().unwrap();
  let state = state(&dir);
  let body = dir.path().join("body.md");
  std::fs::write(&body, "original").unwrap();
  let fixture = Fixture::start().unwrap();
  let endpoint = "/repos/Falconiere/toolu/issues/165/comments";
  fixture
    .route(endpoint, Reply::new(404, r#"{"message":"Not Found"}"#))
    .unwrap();
  let env = recorded::env(&fixture);
  let args = conversation_reply_args(&state, &body);
  let (exit, refused) = run(&args, &on(&fixture, &env));
  assert_eq!(exit, Exit::TempFail, "{refused}");
  assert_eq!(refused["errors"][0]["code"], "api_error");
  let saved = read(&state)["actions"]["replyIntents"]["conversation:5"]["body"]
    .as_str()
    .unwrap()
    .to_owned();
  std::fs::write(&body, "").unwrap();
  fixture
    .route(endpoint, Reply::new(201, r#"{"id":100}"#))
    .unwrap();
  let (exit, empty) = run(&args, &on(&fixture, &env));
  assert_eq!(exit, Exit::Usage, "{empty}");
  assert_eq!(empty["errors"][0]["message"], "reply body is empty");
  assert_eq!(post_count(&fixture), 1);
  std::fs::remove_file(&body).unwrap();
  let (exit, done) = run(&args, &on(&fixture, &env));
  assert_eq!(exit, Exit::Success, "{done}");
  assert_eq!(done["commentId"], 100);
  assert_eq!(post_count(&fixture), 2);
  let requests = fixture.requests().unwrap();
  let posted: serde_json::Value = serde_json::from_slice(&requests[1].body).unwrap();
  assert_eq!(posted["body"], saved);
}
