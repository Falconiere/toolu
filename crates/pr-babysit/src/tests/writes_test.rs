use std::path::PathBuf;
use std::time::Duration;

use serde_json::{Value, json};
use tempfile::TempDir;
use toolu_http_test_support::{Fixture, Reply};
use toolu_runtime::env::Env;

use super::{Slot, Target, reply, resolve};
use crate::{fixture, recorded};

const NOW: &str = "2026-10-11T12:00:00Z";

fn state(dir: &TempDir) -> PathBuf {
  let path = dir.path().join("next.json");
  std::fs::copy(fixture::path("states/toolu-165-initial.json"), &path).unwrap();
  path
}

fn read(path: &PathBuf) -> Value {
  serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn resolved(is_resolved: bool) -> Reply {
  let thread = json!({"id": "PRRT_a", "isResolved": is_resolved});
  Reply::new(200, json!({"data": {"resolveReviewThread": {"thread": thread}}}).to_string())
}

#[test]
fn a_thread_reply_posts_once_records_its_id_and_refuses_a_duplicate() {
  let dir = TempDir::new().unwrap();
  let path = state(&dir);
  let fixture = Fixture::start().unwrap();
  let posted = json!({"id": 7234, "html_url": "https://github.com/Falconiere/toolu/pull/165#c"});
  let endpoint = "/repos/Falconiere/toolu/pulls/165/comments/41/replies";
  fixture.route(endpoint, Reply::new(201, posted.to_string())).unwrap();
  let client = recorded::client(&fixture);
  let env = Env::default();
  let slot = Slot { path: &path, env: &env, now: NOW };
  let to = Target::Thread { thread: "PRRT_a", root: 41, in_reply_to: 42 };
  let done = reply(&client, &slot, &to, "Fixed the empty case.\n").unwrap();
  assert_eq!(done["key"], "thread:PRRT_a@42");
  assert_eq!(done["commentId"], 7234);
  let record = &read(&path)["actions"]["replied"]["thread:PRRT_a@42"];
  assert_eq!(record["commentId"], 7234);
  assert_eq!(record["kind"], "thread");
  assert_eq!(record["at"], NOW);
  let requests = fixture.requests().unwrap();
  let body: Value = serde_json::from_slice(&requests[0].body).unwrap();
  assert_eq!(body, json!({"body": "Fixed the empty case.\n"}));
  assert_eq!(requests[0].method, "POST");

  let duplicate = reply(&client, &slot, &to, "again").err().unwrap();
  assert_eq!(duplicate.code, "duplicate_reply");
  assert_eq!(duplicate.exit(), toolu_protocol::exit::Exit::Failure);
  assert_eq!(fixture.requests().unwrap().len(), 1, "no second request");
}

#[test]
fn conversation_and_review_replies_post_issue_comments_under_their_own_keys() {
  let dir = TempDir::new().unwrap();
  let path = state(&dir);
  let fixture = Fixture::start().unwrap();
  let endpoint = "/repos/Falconiere/toolu/issues/165/comments";
  fixture.route(endpoint, Reply::new(201, r#"{"id": 9}"#)).unwrap();
  let client = recorded::client(&fixture);
  let env = Env::default();
  let slot = Slot { path: &path, env: &env, now: NOW };
  reply(&client, &slot, &Target::Conversation(5), "ok").unwrap();
  let done = reply(&client, &slot, &Target::Review(6), "ok").unwrap();
  assert_eq!(done["url"], Value::Null);
  let replied = &read(&path)["actions"]["replied"];
  assert_eq!(replied["conversation:5"]["kind"], "conversation");
  assert_eq!(replied["review:6"]["kind"], "review");
}

#[test]
fn a_reply_without_a_comment_id_records_nothing() {
  let dir = TempDir::new().unwrap();
  let path = state(&dir);
  let original = std::fs::read(&path).unwrap();
  let fixture = Fixture::start().unwrap();
  fixture.route("/repos/Falconiere/toolu/issues/165/comments", Reply::new(201, "{}")).unwrap();
  let client = recorded::client(&fixture);
  let env = Env::default();
  let slot = Slot { path: &path, env: &env, now: NOW };
  let failure = reply(&client, &slot, &Target::Conversation(5), "ok").err().unwrap();
  assert_eq!(failure.code, "invalid_json");
  assert_eq!(std::fs::read(&path).unwrap(), original);
}

#[test]
fn resolve_records_only_a_confirmed_mutation_and_skips_a_repeat() {
  let dir = TempDir::new().unwrap();
  let path = state(&dir);
  let fixture = Fixture::start().unwrap();
  fixture.route("/graphql", resolved(true)).unwrap();
  let client = recorded::client(&fixture);
  let env = Env::default();
  let slot = Slot { path: &path, env: &env, now: NOW };
  let done = resolve(&client, &slot, "PRRT_a", Duration::ZERO).unwrap();
  assert_eq!(done, json!({"ok": true, "thread": "PRRT_a", "confirmed": true, "attempts": 1}));
  assert_eq!(read(&path)["actions"]["resolved"]["PRRT_a"]["confirmed"], true);
  let again = resolve(&client, &slot, "PRRT_a", Duration::ZERO).unwrap();
  assert_eq!(again["alreadyResolved"], true);
  assert_eq!(again["at"], NOW);
  assert_eq!(fixture.requests().unwrap().len(), 1);
}

#[test]
fn an_unconfirmed_resolve_tries_three_times_and_leaves_the_state_unchanged() {
  let dir = TempDir::new().unwrap();
  let path = state(&dir);
  let original = std::fs::read(&path).unwrap();
  let fixture = Fixture::start().unwrap();
  fixture.route("/graphql", resolved(false)).unwrap();
  let client = recorded::client(&fixture);
  let env = Env::default();
  let slot = Slot { path: &path, env: &env, now: NOW };
  let failure = resolve(&client, &slot, "PRRT_a", Duration::ZERO).err().unwrap();
  assert_eq!(failure.code, "resolve_unconfirmed");
  assert_eq!(failure.exit(), toolu_protocol::exit::Exit::TempFail);
  assert_eq!(failure.extra["lastResponse"]["isResolved"], false);
  assert_eq!(std::fs::read(&path).unwrap(), original);
  assert_eq!(fixture.requests().unwrap().len(), 3);
}
