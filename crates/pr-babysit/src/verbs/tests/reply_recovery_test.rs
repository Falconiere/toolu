use serde_json::json;
use tempfile::TempDir;
use toolu_http_test_support::{Fixture, Reply};
use toolu_protocol::exit::Exit;

use super::tests::{on, state};
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
        Reply::committed_then_dropped(),
      )
      .unwrap();
    let list = "/repos/Falconiere/toolu/issues/165/comments?per_page=100";
    fixture.route(list, Reply::new(200, "[]")).unwrap();
    let env = recorded::env(&fixture);
    let args = [
      "reply",
      "--kind",
      "conversation",
      "--comment-id",
      "5",
      "--state-file",
      state.to_str().unwrap(),
      "--body-file",
      body.to_str().unwrap(),
    ];
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
