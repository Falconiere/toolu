use serde_json::{Value, json};
use toolu_http_test_support::{Fixture, Reply};
use toolu_runtime::env::Env;

use super::settle_issue;
use crate::gate_rig::{settle, trackers};
use crate::support::gh;

#[test]
fn a_merged_issue_is_closed_and_ticked_on_its_epic() {
  let fixture = Fixture::start().expect("fixture");
  let reply = |body: Value| Reply::new(200, body.to_string());
  fixture
    .route(
      "/repos/o/r/issues/5",
      reply(json!({"state": "closed", "title": "Ship it"})),
    )
    .expect("issue");
  fixture
    .route(
      "/repos/o/r/issues/1",
      reply(json!({"body": "- [ ] https://github.com/o/r/issues/5 Ship it\n"})),
    )
    .expect("epic");
  let (env, client, make) = (
    Env::from_pairs([("HOME", "/tmp")]),
    gh(&fixture),
    trackers(&fixture),
  );
  let at = settle(&env, &client, make.as_ref());
  let settled = settle_issue(
    &at,
    "o/r#5",
    "https://github.com/o/r/pull/7",
    Some("o/r#1"),
    None,
  )
  .expect("settled");
  assert_eq!(
    Value::Object(settled),
    json!({"issue": "closed-by-pr", "epic_ticked": true})
  );
  let patch = fixture
    .requests()
    .expect("requests")
    .into_iter()
    .find(|request| request.method == "PATCH")
    .expect("tick");
  let body: Value = serde_json::from_slice(&patch.body).expect("body");
  assert_eq!(
    body["body"],
    "- [x] https://github.com/o/r/issues/5 Ship it\n"
  );
}

#[test]
fn without_an_epic_only_the_issue_is_closed() {
  let fixture = Fixture::start().expect("fixture");
  for path in ["/repos/o/r/issues/5", "/repos/o/r/issues/5/comments"] {
    fixture
      .route(path, Reply::new(200, r#"{"state":"open"}"#))
      .expect("route");
  }
  let (env, client, make) = (
    Env::from_pairs([("HOME", "/tmp")]),
    gh(&fixture),
    trackers(&fixture),
  );
  let at = settle(&env, &client, make.as_ref());
  let settled =
    settle_issue(&at, "o/r#5", "https://github.com/o/r/pull/7", None, None).expect("settled");
  assert_eq!(Value::Object(settled), json!({"issue": "closed-manually"}));
  let comment = fixture
    .requests()
    .expect("requests")
    .into_iter()
    .find(|request| request.method == "POST")
    .expect("note");
  let body: Value = serde_json::from_slice(&comment.body).expect("body");
  assert_eq!(body["body"], "Delivered in https://github.com/o/r/pull/7.");
}
