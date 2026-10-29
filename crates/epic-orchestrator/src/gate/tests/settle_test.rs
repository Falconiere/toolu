use serde_json::{Value, json};
use std::path::Path;
use toolu_http_test_support::{Fixture, Reply};
use toolu_runtime::env::Env;

use super::settle_issue;
use crate::gate_rig::{settle, trackers};
use crate::support::gh;

fn settle_on_fixture(
  fixture: &Fixture,
  state: &Path,
  epic: Option<&str>,
) -> serde_json::Map<String, Value> {
  let env = Env::from_pairs([("HOME", "/tmp")]);
  let client = gh(fixture);
  let make = trackers(fixture);
  let at = settle(&env, &client, make.as_ref());
  settle_issue(
    &at,
    "o/r#5",
    "https://github.com/o/r/pull/7",
    epic,
    (None, state),
  )
  .expect("settled")
}

#[test]
fn a_merged_issue_is_closed_and_ticked_on_its_epic() {
  let fixture = Fixture::start().expect("fixture");
  let state = tempfile::tempdir().expect("state");
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
  let settled = settle_on_fixture(&fixture, state.path(), Some("o/r#1"));
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
  let state = tempfile::tempdir().expect("state");
  for path in ["/repos/o/r/issues/5", "/repos/o/r/issues/5/comments"] {
    fixture
      .route(path, Reply::new(200, r#"{"state":"open"}"#))
      .expect("route");
  }
  fixture
    .route(
      "/repos/o/r/issues/5/comments?per_page=100",
      Reply::new(200, "[]"),
    )
    .expect("list");
  let settled = settle_on_fixture(&fixture, state.path(), None);
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
