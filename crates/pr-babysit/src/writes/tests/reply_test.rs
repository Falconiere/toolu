use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use tempfile::TempDir;
use toolu_http_test_support::{Fixture, Reply};
use toolu_runtime::env::Env;

use super::{Slot, Target, reply};
use crate::{fixture, recorded};

const NOW: &str = "2026-10-28T12:00:00Z";

pub(super) fn state(dir: &Path) -> PathBuf {
  let path = dir.join("next.json");
  std::fs::copy(fixture::path("states/toolu-165-initial.json"), &path).unwrap();
  path
}

pub(super) fn read(path: &Path) -> Value {
  serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

pub(super) fn slot<'a>(path: &'a Path, env: &'a Env) -> Slot<'a> {
  Slot {
    path,
    env,
    now: NOW,
  }
}

pub(super) fn posts(fixture: &Fixture) -> usize {
  fixture
    .requests()
    .unwrap()
    .iter()
    .filter(|request| request.method == "POST")
    .count()
}

fn paths(to: &Target<'_>) -> (&'static str, &'static str) {
  match to {
    Target::Thread { .. } => (
      "/repos/Falconiere/toolu/pulls/165/comments/41/replies",
      "/repos/Falconiere/toolu/pulls/165/comments?per_page=100",
    ),
    Target::Conversation(_) | Target::Review(_) => (
      "/repos/Falconiere/toolu/issues/165/comments",
      "/repos/Falconiere/toolu/issues/165/comments?per_page=100",
    ),
  }
}

fn lose_and_capture(fixture: &Fixture, path: &Path, env: &Env, to: &Target<'_>) -> String {
  let (post_path, list_path) = paths(to);
  fixture
    .route(post_path, Reply::dropped_recorded())
    .unwrap();
  fixture.route(list_path, Reply::new(200, "[]")).unwrap();
  let first = reply(&recorded::client(fixture), &slot(path, env), to, "fixed").unwrap_err();
  assert_eq!(first.code, "reply_uncertain");
  let key = to.key();
  let intent = &read(path)["actions"]["replyIntents"][&key];
  assert_eq!(intent["status"], "sending");
  intent["body"].as_str().unwrap().to_owned()
}

fn serve_later_page(fixture: &Fixture, to: &Target<'_>, index: usize, body: &str) {
  let (_, list_path) = paths(to);
  let second_path = format!("{list_path}&page=2");
  let next = fixture.url(&second_path);
  fixture
    .route(
      list_path,
      Reply::new(200, "[]").header("Link", &format!("<{next}>; rel=\"next\"")),
    )
    .unwrap();
  let remote = json!({
    "id": 9000 + index,
    "body": body,
    "html_url": format!("https://github.com/Falconiere/toolu/pull/165#comment-{index}"),
    "in_reply_to_id": if matches!(to, Target::Thread { .. }) { Some(41) } else { None },
  });
  fixture
    .route(&second_path, Reply::new(200, json!([remote]).to_string()))
    .unwrap();
}

fn assert_reconciled(fixture: &Fixture, path: &Path, env: &Env, to: &Target<'_>, index: usize) {
  let key = to.key();
  let done = reply(
    &recorded::client(fixture),
    &slot(path, env),
    to,
    "changed body",
  )
  .unwrap();
  assert_eq!(done["commentId"], 9000 + index);
  assert_eq!(
    read(path)["actions"]["replied"][&key]["commentId"],
    9000 + index
  );
  assert!(read(path)["actions"]["replyIntents"].get(&key).is_none());
  assert_eq!(
    reply(&recorded::client(fixture), &slot(path, env), to, "again")
      .unwrap_err()
      .code,
    "duplicate_reply"
  );
}

#[test]
fn a_lost_response_reconciles_every_reply_target_from_a_later_page() {
  let dir = TempDir::new().unwrap();
  let path = state(dir.path());
  let fixture = Fixture::start().unwrap();
  let env = Env::default();
  let targets = [
    Target::Thread {
      thread: "PRRT_a",
      root: 41,
      in_reply_to: 42,
    },
    Target::Conversation(5),
    Target::Review(6),
  ];
  for (index, to) in targets.iter().enumerate() {
    let body = lose_and_capture(&fixture, &path, &env, to);
    serve_later_page(&fixture, to, index, &body);
    assert_reconciled(&fixture, &path, &env, to, index);
  }
  assert_eq!(posts(&fixture), 3);
}

fn assert_duplicate_evidence_uncertain(fixture: &Fixture, path: &Path, env: &Env, to: &Target<'_>) {
  let list = "/repos/Falconiere/toolu/issues/165/comments?per_page=100";
  let body = read(path)["actions"]["replyIntents"]["conversation:5"]["body"]
    .as_str()
    .unwrap()
    .to_owned();
  fixture
    .route(
      list,
      Reply::new(
        200,
        json!([
          {"id": 1, "body": body}, {"id": 2, "body": body}
        ])
        .to_string(),
      ),
    )
    .unwrap();
  let ambiguous = reply(&recorded::client(fixture), &slot(path, env), to, "fixed").unwrap_err();
  assert_eq!(ambiguous.code, "reply_uncertain");
  assert_eq!(
    ambiguous.extra["reason"],
    "multiple matching remote comments"
  );
  assert_eq!(posts(fixture), 1);
}

#[test]
fn an_unresolved_send_never_posts_again_and_keeps_its_intent() {
  let dir = TempDir::new().unwrap();
  let path = state(dir.path());
  let fixture = Fixture::start().unwrap();
  let post = "/repos/Falconiere/toolu/issues/165/comments";
  let list = "/repos/Falconiere/toolu/issues/165/comments?per_page=100";
  fixture
    .route(post, Reply::dropped_recorded())
    .unwrap();
  fixture.route(list, Reply::new(200, "[]")).unwrap();
  let env = Env::default();
  let to = Target::Conversation(5);
  let first = reply(
    &recorded::client(&fixture),
    &slot(&path, &env),
    &to,
    "fixed",
  )
  .unwrap_err();
  assert_eq!(first.code, "reply_uncertain");
  let second = reply(
    &recorded::client(&fixture),
    &slot(&path, &env),
    &to,
    "fixed",
  )
  .unwrap_err();
  assert_eq!(second.code, "reply_uncertain");
  assert_eq!(second.exit(), toolu_protocol::exit::Exit::TempFail);
  assert_eq!(posts(&fixture), 1);
  assert_eq!(
    read(&path)["actions"]["replyIntents"]["conversation:5"]["status"],
    "sending"
  );
  assert_duplicate_evidence_uncertain(&fixture, &path, &env, &to);
}

#[test]
fn a_definitive_refusal_leaves_a_prepared_intent_that_can_send() {
  let dir = TempDir::new().unwrap();
  let path = state(dir.path());
  let fixture = Fixture::start().unwrap();
  let post = "/repos/Falconiere/toolu/issues/165/comments";
  fixture
    .route(post, Reply::new(404, r#"{"message":"Not Found"}"#))
    .unwrap();
  let env = Env::default();
  let to = Target::Review(6);
  let refused = reply(
    &recorded::client(&fixture),
    &slot(&path, &env),
    &to,
    "fixed",
  )
  .unwrap_err();
  assert_eq!(refused.code, "api_error");
  assert_eq!(
    read(&path)["actions"]["replyIntents"]["review:6"]["status"],
    "prepared"
  );
  fixture
    .route(post, Reply::new(201, r#"{"id":55}"#))
    .unwrap();
  let done = reply(
    &recorded::client(&fixture),
    &slot(&path, &env),
    &to,
    "different",
  )
  .unwrap();
  assert_eq!(done["commentId"], 55);
  assert_eq!(posts(&fixture), 2);
}

#[test]
fn an_oversized_body_fails_before_an_intent_or_post_and_a_shorter_one_sends() {
  let dir = TempDir::new().unwrap();
  let path = state(dir.path());
  let original = std::fs::read(&path).unwrap();
  let fixture = Fixture::start().unwrap();
  let env = Env::default();
  let to = Target::Conversation(5);
  let oversized = "x".repeat(65_536);
  let failure = reply(
    &recorded::client(&fixture),
    &slot(&path, &env),
    &to,
    &oversized,
  )
  .unwrap_err();
  assert_eq!(failure.code, "usage");
  assert_eq!(std::fs::read(&path).unwrap(), original);
  assert_eq!(posts(&fixture), 0);
  fixture
    .route(
      "/repos/Falconiere/toolu/issues/165/comments",
      Reply::new(201, r#"{"id":90}"#),
    )
    .unwrap();
  assert_eq!(
    reply(
      &recorded::client(&fixture),
      &slot(&path, &env),
      &to,
      "short"
    )
    .unwrap()["commentId"],
    90
  );
  assert_eq!(posts(&fixture), 1);
}
