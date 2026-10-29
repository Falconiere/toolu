use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tempfile::TempDir;
use toolu_http_test_support::{Fixture, Reply};
use toolu_runtime::env::Env;

use super::tests::{posts, read, slot, state};
use super::{Target, reply};
use crate::recorded;

fn start_reply(fixture: &Fixture, path: &Path) -> thread::JoinHandle<crate::error::Result<Value>> {
  let client = recorded::client(fixture);
  let path = path.to_path_buf();
  thread::spawn(move || {
    let env = Env::default();
    reply(
      &client,
      &slot(&path, &env),
      &Target::Conversation(5),
      "fixed",
    )
  })
}

fn wait_for_post(fixture: &Fixture) {
  let started = Instant::now();
  while posts(fixture) == 0 && started.elapsed() < Duration::from_secs(3) {
    thread::sleep(Duration::from_millis(5));
  }
  assert_eq!(posts(fixture), 1);
}

#[test]
fn a_failed_receipt_write_recovers_the_original_remote_comment() {
  let dir = TempDir::new().unwrap();
  let state_dir = dir.path().join("state");
  std::fs::create_dir(&state_dir).unwrap();
  let path = state(&state_dir);
  let fixture = Fixture::start().unwrap();
  let post = "/repos/Falconiere/toolu/issues/165/comments";
  let list = "/repos/Falconiere/toolu/issues/165/comments?per_page=100";
  fixture
    .route(
      post,
      Reply::new(201, r#"{"id":77}"#).delayed(Duration::from_millis(700)),
    )
    .unwrap();
  let worker = start_reply(&fixture, &path);
  wait_for_post(&fixture);
  let parked = dir.path().join("parked");
  std::fs::rename(&state_dir, &parked).unwrap();
  std::fs::write(&state_dir, "blocks atomic replacement").unwrap();
  let failed = worker.join().unwrap().unwrap_err();
  assert_eq!(failed.code, "invalid_json");
  std::fs::remove_file(&state_dir).unwrap();
  std::fs::remove_dir_all(parked.join("next.json.lock")).unwrap();
  std::fs::rename(&parked, &state_dir).unwrap();
  let intent = &read(&path)["actions"]["replyIntents"]["conversation:5"];
  assert_eq!(intent["status"], "sending");
  let body = intent["body"].as_str().unwrap().to_owned();
  fixture
    .route(
      list,
      Reply::new(
        200,
        json!([{"id":77,"body":body,"html_url":"https://github.com/receipt"}]).to_string(),
      ),
    )
    .unwrap();
  let env = Env::default();
  let done = reply(
    &recorded::client(&fixture),
    &slot(&path, &env),
    &Target::Conversation(5),
    "changed",
  )
  .unwrap();
  assert_eq!(done["commentId"], 77);
  assert_eq!(posts(&fixture), 1);
}

#[test]
fn concurrent_same_target_is_locked_and_another_target_remains_usable() {
  let dir = TempDir::new().unwrap();
  let path = state(dir.path());
  let fixture = Fixture::start().unwrap();
  let post = "/repos/Falconiere/toolu/issues/165/comments";
  fixture
    .route(
      post,
      Reply::new(201, r#"{"id":88}"#).delayed(Duration::from_millis(500)),
    )
    .unwrap();
  let worker = start_reply(&fixture, &path);
  wait_for_post(&fixture);
  let env = Env::default();
  let locked = reply(
    &recorded::client(&fixture),
    &slot(&path, &env),
    &Target::Conversation(5),
    "fixed",
  )
  .unwrap_err();
  assert_eq!(locked.code, "locked");
  assert_eq!(worker.join().unwrap().unwrap()["commentId"], 88);
  assert_eq!(
    reply(
      &recorded::client(&fixture),
      &slot(&path, &env),
      &Target::Conversation(5),
      "again"
    )
    .unwrap_err()
    .code,
    "duplicate_reply"
  );
  fixture
    .route(post, Reply::new(201, r#"{"id":89}"#))
    .unwrap();
  let other = reply(
    &recorded::client(&fixture),
    &slot(&path, &env),
    &Target::Review(6),
    "other",
  )
  .unwrap();
  assert_eq!(other["commentId"], 89);
  assert_eq!(posts(&fixture), 2);
}
