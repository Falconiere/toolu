use std::path::{Path, PathBuf};
use std::time::Instant;

use serde_json::{Value, json};
use tempfile::TempDir;
use toolu_http_test_support::{Fixture, Reply};
use toolu_protocol::exit::Exit;
use toolu_runtime::env::Env;

use crate::verbs::World;
use crate::verbs::tests::{run, world};
use crate::{fixture, recorded};

const NOW: &str = "2026-10-11T12:00:00Z";

fn read(path: &Path) -> Value {
  serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn state(dir: &TempDir) -> PathBuf {
  let path = dir.path().join("falconiere-toolu-165.json");
  std::fs::copy(fixture::path("states/toolu-165-initial.json"), &path).unwrap();
  path
}

fn on<'a>(fixture: &Fixture, env: &'a Env) -> World<'a> {
  let mut world = world(env, None);
  world.github = Ok(recorded::config(fixture));
  world
}

/// Serve the recorded Falconiere/toolu#115 round once.
fn serve_115(fixture: &Fixture) {
  let sha = recorded::snapshot()["head"]["sha"]
    .as_str()
    .unwrap()
    .to_owned();
  recorded::serve(
    fixture,
    &[&sha, &sha],
    recorded::replies(&recorded::round()),
  );
}

fn pr_115<'a>(verb: &'a str, state: &'a str) -> Vec<&'a str> {
  let pr = [
    "--repo",
    "Falconiere/toolu",
    "--pr",
    "115",
    "--page-size",
    "2",
  ];
  [&[verb][..], &pr, &["--state-file", state]].concat()
}

#[test]
fn tick_reduces_a_recorded_snapshot_into_the_state_file() {
  let dir = TempDir::new().unwrap();
  let state = dir.path().join("s.json");
  let snapshot = fixture::path("snapshots/toolu-165.json");
  let env = Env::default();
  let args = [
    "tick",
    "--repo",
    "Falconiere/toolu",
    "--pr",
    "165",
    "--state-file",
    state.to_str().unwrap(),
    "--snapshot-in",
    snapshot.to_str().unwrap(),
    "--now",
    NOW,
  ];
  let (exit, result) = run(&args, &world(&env, None));
  assert_eq!(exit, Exit::Success);
  assert_eq!(result["decision"], "escalate");
  assert_eq!(read(&state)["lastUpdate"], NOW);
}

#[test]
fn tick_refuses_a_malformed_repo_or_time() {
  let env = Env::default();
  let world = world(&env, None);
  let base = ["tick", "--pr", "1", "--state-file", "s.json"];
  let (exit, error) = run(&[&base[..], &["--repo", "not a repo"]].concat(), &world);
  assert_eq!(exit, Exit::Usage);
  assert_eq!(
    error["errors"][0]["message"],
    "--repo <owner/repo> required"
  );
  let late = [
    &base[..],
    &["--repo", "o/r", "--now", "soon", "--snapshot-in", "x"],
  ]
  .concat();
  let (exit, error) = run(&late, &world);
  assert_eq!(exit, Exit::Usage);
  assert_eq!(
    error["errors"][0]["message"],
    "--now must be YYYY-MM-DDTHH:MM:SSZ"
  );
}

#[test]
fn a_tick_without_a_token_is_gh_unavailable() {
  let dir = TempDir::new().unwrap();
  let state = dir.path().join("s.json");
  let env = Env::from_pairs([("PATH", "/nonexistent")]);
  let (exit, error) = run(&pr_115("tick", state.to_str().unwrap()), &world(&env, None));
  assert_eq!(exit, Exit::Unavailable);
  assert_eq!(error["errors"][0]["code"], "gh_unavailable");
}

#[test]
fn collect_writes_the_snapshot_to_out_and_prints_its_path() {
  let dir = TempDir::new().unwrap();
  let out = dir.path().join("snap.json");
  let fixture = Fixture::start().unwrap();
  serve_115(&fixture);
  let env = recorded::env(&fixture);
  let args = [
    "collect",
    "--repo",
    "Falconiere/toolu",
    "--pr",
    "115",
    "--page-size",
    "2",
  ];
  let (exit, printed) = run(
    &[&args[..], &["--out", out.to_str().unwrap()]].concat(),
    &on(&fixture, &env),
  );
  assert_eq!(exit, Exit::Success);
  assert_eq!(printed, Value::String(out.display().to_string()));
  let (mut written, mut expected) = (read(&out), recorded::snapshot());
  written.as_object_mut().unwrap().remove("collectedAt");
  expected.as_object_mut().unwrap().remove("collectedAt");
  assert_eq!(written, expected);
}

#[test]
fn wait_with_no_time_checks_github_once_and_returns_what_needs_the_agent() {
  let dir = TempDir::new().unwrap();
  let state = dir.path().join("s.json");
  let fixture = Fixture::start().unwrap();
  serve_115(&fixture);
  let env = recorded::env(&fixture);
  let args = [
    &pr_115("wait", state.to_str().unwrap())[..],
    &["--max-seconds", "0"],
  ]
  .concat();
  let (exit, result) = run(&args, &on(&fixture, &env));
  assert_eq!(exit, Exit::Success);
  assert_eq!(result["decision"], "escalate");
  assert!(
    read(&state)["schedule"]["nextCheckAt"].is_u64(),
    "a scheduled check"
  );
}

#[test]
fn wait_before_the_next_check_waits_out_its_time_without_calling_github() {
  let dir = TempDir::new().unwrap();
  let state = state(&dir);
  let mut document = read(&state);
  document["schedule"] = json!({"anchor": 4_102_444_620_u64, "nextCheckAt": 4_102_444_800_u64});
  std::fs::write(&state, document.to_string()).unwrap();
  let fixture = Fixture::start().unwrap();
  let env = recorded::env(&fixture);
  let started = Instant::now();
  let args = [
    "wait",
    "--repo",
    "Falconiere/toolu",
    "--pr",
    "165",
    "--max-seconds",
    "1",
  ];
  let (exit, waited) = run(
    &[&args[..], &["--state-file", state.to_str().unwrap()]].concat(),
    &on(&fixture, &env),
  );
  assert_eq!(exit, Exit::Success);
  assert!(started.elapsed().as_millis() >= 900);
  assert_eq!(waited["state"], "waiting");
  assert_eq!(waited["slot"], "falconiere-toolu-165");
  assert_eq!(waited["nextCheckAt"], "2100-01-01T00:00:00Z");
  assert_eq!(
    waited["resultPath"],
    crate::state::sibling(&state, ".result.json")
      .display()
      .to_string()
  );
  assert_eq!(fixture.requests().unwrap().len(), 0, "no GitHub call");
}

#[test]
fn reply_posts_a_conversation_reply_from_the_body_file() {
  let dir = TempDir::new().unwrap();
  let state = state(&dir);
  let body = dir.path().join("body.md");
  std::fs::write(&body, "Thanks, fixed.\n").unwrap();
  let fixture = Fixture::start().unwrap();
  let endpoint = "/repos/Falconiere/toolu/issues/165/comments";
  fixture
    .route(endpoint, Reply::new(201, r#"{"id": 9}"#))
    .unwrap();
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
  let (exit, done) = run(&args, &on(&fixture, &env));
  assert_eq!(exit, Exit::Success, "{done}");
  assert_eq!(done["commentId"], 9);
  assert_eq!(
    read(&state)["actions"]["replied"]["conversation:5"]["commentId"],
    9
  );
}

fn reply_error(dir: &TempDir, body: &str, kind: &[&str]) -> Value {
  let file = dir.path().join("body.md");
  std::fs::write(&file, body).unwrap();
  let env = Env::default();
  let base = [
    "reply",
    "--state-file",
    "s.json",
    "--body-file",
    file.to_str().unwrap(),
  ];
  let (exit, error) = run(&[&base[..], kind].concat(), &world(&env, None));
  assert_eq!(exit, Exit::Usage);
  error["errors"][0]["message"].clone()
}

#[test]
fn reply_refuses_an_empty_or_oversized_body_and_a_missing_target() {
  let dir = TempDir::new().unwrap();
  let thread = [
    "--kind",
    "thread",
    "--thread",
    "PRRT_a",
    "--root-comment",
    "1",
  ];
  assert_eq!(
    reply_error(&dir, "", &["--kind", "review"]),
    "reply body is empty"
  );
  let long = "x".repeat(65_537);
  let limit = "reply body exceeds GitHub's 65536-character limit";
  assert_eq!(reply_error(&dir, &long, &["--kind", "review"]), limit);
  let missing = "--in-reply-to <id> required for --kind thread";
  assert_eq!(reply_error(&dir, "ok", &thread), missing);
  let bad = [
    "--kind",
    "thread",
    "--thread",
    "PRRT a",
    "--root-comment",
    "1",
  ];
  assert_eq!(
    reply_error(&dir, "ok", &bad),
    "--thread <graphqlId> required for --kind thread"
  );
  let review = "--review-id <id> required for --kind review";
  assert_eq!(reply_error(&dir, "ok", &["--kind", "review"]), review);
}

#[test]
fn resolve_refuses_a_thread_id_that_is_not_a_graphql_id() {
  let env = Env::default();
  let args = ["resolve", "--state-file", "s.json", "--thread", "PRRT;rm"];
  let (exit, error) = run(&args, &world(&env, None));
  assert_eq!(exit, Exit::Usage);
  assert_eq!(
    error["errors"][0]["message"],
    "--thread <graphqlId> required"
  );
}

#[test]
fn resolve_resolves_the_thread_and_records_it() {
  let dir = TempDir::new().unwrap();
  let state = state(&dir);
  let fixture = Fixture::start().unwrap();
  let thread = json!({"id": "PRRT_a", "isResolved": true});
  let reply = json!({"data": {"resolveReviewThread": {"thread": thread}}});
  fixture
    .route("/graphql", Reply::new(200, reply.to_string()))
    .unwrap();
  let env = recorded::env(&fixture);
  let args = [
    "resolve",
    "--state-file",
    state.to_str().unwrap(),
    "--thread",
    "PRRT_a",
  ];
  let (exit, done) = run(&args, &on(&fixture, &env));
  assert_eq!(exit, Exit::Success, "{done}");
  assert!(read(&state)["actions"]["resolved"].get("PRRT_a").is_some());
}
