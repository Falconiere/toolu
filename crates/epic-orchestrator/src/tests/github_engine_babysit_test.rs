use std::path::Path;

use serde_json::{Value, json};
use toolu_github::Config;
use toolu_http_test_support::{Fixture, Reply};
use toolu_pr_babysit::GitHubTick;
use toolu_runtime::env::Env;

use super::fixture::{engine, rest_bodies};
use crate::journal::Record;
use crate::model::{Action, Step, fresh};
use crate::server::Engine;

pub(super) const SLOT_MS: u64 = 180_000;

fn verdict(name: &str) -> String {
  let file = Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../../fixtures/pr-babysit/verdicts")
    .join(name);
  std::fs::read_to_string(file).expect("verdict fixture")
}

pub(super) fn bot_comment(name: &str, updated: &str) -> Value {
  json!([{"id": 555, "body": verdict(name), "user": {"login": "github-actions", "type": "Bot"},
          "created_at": "2026-10-11T10:00:00Z", "updated_at": updated,
          "html_url": "https://github.com/o/r/pull/1#issuecomment-555"}])
}

pub(super) fn ci_thread(resolved: bool) -> Value {
  let comment = json!({"id": "PRRC_1", "databaseId": 101, "body": "**medium**: off by one",
    "author": {"login": "github-actions", "__typename": "Bot"},
    "createdAt": "2026-10-11T10:00:00Z", "url": "https://github.com/o/r/pull/1#r101"});
  json!({"id": "PRRT_ci", "isResolved": resolved, "isOutdated": false, "path": "src/lib.rs",
    "line": 12, "comments": {"pageInfo": {"hasNextPage": false, "endCursor": null}, "nodes": [comment]}})
}

/// One reply that answers both the PR query and the threads query.
pub(super) fn graphql(conclusion: &str, threads: &[Value]) -> String {
  let check = json!({"__typename": "CheckRun", "name": "test", "status": "COMPLETED",
    "conclusion": conclusion, "detailsUrl": "https://ci.example/1",
    "startedAt": "2026-10-11T09:00:00Z", "completedAt": "2026-10-11T09:10:00Z",
    "checkSuite": {"workflowRun": {"workflow": {"name": "tests"}}}});
  json!({"data": {"rateLimit": {"cost": 1}, "repository": {"pullRequest": {
    "number": 1, "title": "feat: one", "url": "https://github.com/o/r/pull/1",
    "author": {"login": "worker"}, "state": "OPEN", "baseRefName": "main",
    "headRefName": "feat/one", "headRefOid": "abc", "mergeable": "MERGEABLE",
    "reviewDecision": null,
    "commits": {"nodes": [{"commit": {"statusCheckRollup": {"contexts": {"nodes": [check]}}}}]},
    "reviewThreads": {"pageInfo": {"hasNextPage": false, "endCursor": null}, "nodes": threads}
  }}}})
  .to_string()
}

/// Every route the engine's probe and pr-babysit's collection read.
fn serve(fixture: &Fixture, graphql_body: String, comments: &Value) {
  for (path, body) in rest_bodies(comments.to_string()) {
    fixture
      .route(&path, Reply::new(200, body))
      .expect("REST route");
  }
  fixture
    .route("/graphql", Reply::new(200, graphql_body))
    .expect("GraphQL route");
}

/// pr-babysit's own tick, reading the fixture's API.
pub(super) fn babysit(fixture: &Fixture) -> GitHubTick {
  let mut config = Config::scheduled();
  config.api_url = fixture.url("");
  config.http.test_root_ca_der = Some(fixture.root_ca_der().to_vec());
  let env = Env::from_pairs([
    ("GH_TOKEN", "engine-token"),
    ("HTTPS_PROXY", &fixture.proxy_url()),
  ]);
  GitHubTick::new(config, env)
}

pub(super) fn journal(engine: &Engine) -> Vec<&Record> {
  let rows = engine.world.outbox.iter().filter_map(|step| match step {
    Step::Journal(row) => Some(row),
    Step::Call { .. }
    | Step::Status { .. }
    | Step::Issue { .. }
    | Step::Prompt { .. }
    | Step::Notify { .. } => None,
  });
  rows.collect()
}

pub(super) fn prompts(engine: &Engine) -> Vec<&Record> {
  journal(engine)
    .into_iter()
    .filter(|row| row.kind == "prompt")
    .collect()
}

fn assert_no_babysit_work(engine: &mut Engine) {
  assert!(
    !engine
      .world
      .attention
      .iter()
      .any(|item| item.kind == "babysit")
  );
  assert!(
    !engine
      .world
      .outbox
      .iter()
      .any(|step| matches!(step, Step::Prompt { .. }))
  );
  engine.flush().expect("flush journal");
  let rows = crate::journal::tail(&engine.paths.journal_dir(), engine.now).expect("journal");
  assert!(
    rows
      .iter()
      .all(|row| !(row.kind == "judgment" && row.name == "babysit"))
  );
  assert!(
    rows
      .iter()
      .all(|row| !(row.kind == "prompt" && row.name == "babysit"))
  );
}

fn run_slots(engine: &mut Engine, tick: &GitHubTick, slots: std::ops::Range<u64>) {
  for slot in slots {
    engine.world.now_ms = slot * SLOT_MS;
    engine.github_tick(tick);
  }
}

#[test]
fn merged_pr_stage_skips_babysit_even_for_a_stale_immediate_watch() {
  let fixture = Fixture::start().expect("fixture");
  serve(&fixture, graphql("SUCCESS", &[]), &json!([]));
  let (_temp, mut engine, _) = engine(&fixture);
  let tick = babysit(&fixture);
  engine.world.issues.get_mut("one").expect("issue").pending =
    Some(fresh(Action::Merge, "merge-token".into()));
  let _steps = crate::commit::applied(&mut engine.world, "one", Some(Action::Merge));
  let _steps = crate::logic::finish_action(&mut engine.world, "one");
  assert_eq!(engine.world.issues["one"].stage, "cleaning");
  assert_eq!(
    engine.world.issues["one"]
      .pending
      .as_ref()
      .map(|p| p.action),
    Some(Action::Cleanup)
  );
  engine
    .world
    .watches
    .get_mut("pr:o/r#1")
    .expect("stale watch")
    .request_immediate();
  engine.github_tick(&tick);
  let requests = fixture.requests().expect("requests");
  assert!(
    requests
      .iter()
      .any(|request| request.path == "/repos/o/r/pulls/1")
  );
  assert!(requests.iter().all(|request| request.path != "/graphql"));
  assert_no_babysit_work(&mut engine);
}

#[test]
fn merged_pr_tick_enters_cleanup_without_babysit_attention_or_prompt() {
  let fixture = Fixture::start().expect("fixture");
  let mut merged: Value = serde_json::from_str(&graphql("SUCCESS", &[])).expect("GraphQL response");
  merged["data"]["repository"]["pullRequest"]["state"] = json!("MERGED");
  serve(&fixture, merged.to_string(), &json!([]));
  let (_temp, mut engine, _) = engine(&fixture);
  let tick = babysit(&fixture);
  engine.github_tick(&tick);
  let requests = fixture.requests().expect("requests");
  assert!(requests.iter().any(|request| request.path == "/graphql"));
  assert_eq!(engine.world.issues["one"].stage, "cleaning");
  assert_eq!(
    engine.world.issues["one"]
      .pending
      .as_ref()
      .map(|p| p.action),
    Some(Action::Cleanup)
  );
  assert!(
    journal(&engine)
      .iter()
      .any(|row| row.name == "github-pr-merged")
  );
  assert_no_babysit_work(&mut engine);
}

#[test]
fn babysit_a_green_approved_pr_reaches_ready_without_a_prompt() {
  let fixture = Fixture::start().expect("fixture");
  serve(
    &fixture,
    graphql("SUCCESS", &[ci_thread(true)]),
    &bot_comment("pr122-verdict-approved.txt", "2026-10-11T10:05:00Z"),
  );
  let (_temp, mut engine, _) = engine(&fixture);
  let tick = babysit(&fixture);
  run_slots(&mut engine, &tick, 0..1);
  assert_eq!(engine.world.issues["one"].phase, "ready");
  assert!(
    journal(&engine)
      .iter()
      .any(|row| row.name == "github-babysit-ready")
  );
  assert_eq!(prompts(&engine).len(), 0);
}

#[test]
fn babysit_one_ci_reviewer_thread_prompts_once_then_reaches_ready() {
  let fixture = Fixture::start().expect("fixture");
  let changes = bot_comment("pr120-verdict-changes.txt", "2026-10-11T10:05:00Z");
  serve(&fixture, graphql("SUCCESS", &[ci_thread(false)]), &changes);
  let (_temp, mut engine, _) = engine(&fixture);
  let tick = babysit(&fixture);
  run_slots(&mut engine, &tick, 0..5);
  let sent = prompts(&engine);
  assert_eq!(sent.len(), 1, "{sent:?}");
  assert_eq!(sent[0].name, "babysit");
  assert!(
    sent[0]
      .note
      .contains("- thread PRRT_ci src/lib.rs:12 (ci_reviewer)"),
    "{}",
    sent[0].note
  );
  assert!(
    engine
      .world
      .outbox
      .iter()
      .any(|step| matches!(step, Step::Prompt { key, .. } if key == "one"))
  );
  assert_eq!(engine.world.issues["one"].phase, "babysit");
  let approved = bot_comment("pr122-verdict-approved.txt", "2026-10-11T10:30:00Z");
  serve(&fixture, graphql("SUCCESS", &[ci_thread(true)]), &approved);
  run_slots(&mut engine, &tick, 5..6);
  assert_eq!(engine.world.issues["one"].phase, "ready");
  assert_eq!(prompts(&engine).len(), 1, "no prompt after the fix");
}

#[test]
fn babysit_six_hours_of_red_ci_tick_every_180_seconds_and_prompt_once() {
  let fixture = Fixture::start().expect("fixture");
  let approved = bot_comment("pr122-verdict-approved.txt", "2026-10-11T10:05:00Z");
  serve(&fixture, graphql("FAILURE", &[]), &approved);
  let (_temp, mut engine, _) = engine(&fixture);
  let tick = babysit(&fixture);
  run_slots(&mut engine, &tick, 0..121);
  let checks: Vec<u64> = engine
    .world
    .watches
    .values()
    .filter_map(|watch| watch.last_at_ms)
    .collect();
  assert_eq!(checks, [120 * SLOT_MS]);
  let ticks = journal(&engine)
    .iter()
    .filter(|row| row.name == "github-check")
    .count();
  assert_eq!(ticks, 121, "one tick per slot, no burst");
  let sent = prompts(&engine);
  assert_eq!(sent.len(), 1);
  assert!(sent[0].note.contains("(ci_failed)"), "{}", sent[0].note);
  assert_eq!(engine.world.issues["one"].phase, "babysit");
}
