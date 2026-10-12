use serde_json::{Value, json};
use toolu_http_test_support::{Fixture, Reply};
use toolu_runtime::env::Env;

use super::{Mode, StepJob, job, step};
use crate::gate_rig::{PrState, serve_pr};
use crate::support::{FAST, reach};

fn input(fixture: &Fixture, mode: Mode) -> StepJob {
  StepJob {
    repo: "o/r".to_owned(),
    pr: 7,
    key: "k".to_owned(),
    mode,
    worktree: None,
    lease: None,
    env: Env::from_pairs([("PATH", "/usr/bin:/bin")]),
    reach: reach(fixture, FAST),
    trace: None,
  }
}

const HEAD: Mode = Mode::Head { parked: true };

fn served(pr: &PrState) -> Fixture {
  let fixture = Fixture::start().expect("fixture");
  serve_pr(&fixture, pr);
  fixture
}

fn pending() -> Value {
  json!([{"__typename": "CheckRun", "name": "test", "status": "IN_PROGRESS", "conclusion": null}])
}

fn failing() -> Value {
  json!([{"__typename": "CheckRun", "name": "test", "status": "COMPLETED", "conclusion": "FAILURE"}])
}

/// The GraphQL bodies and the merge calls the step sent.
fn sent(fixture: &Fixture) -> (Vec<Value>, Vec<Value>) {
  let requests = fixture.requests().expect("requests");
  let body = |raw: &[u8]| serde_json::from_slice::<Value>(raw).unwrap_or(Value::Null);
  let graphql = requests
    .iter()
    .filter(|request| request.path == "/graphql")
    .map(|request| body(&request.body))
    .filter(|query| query["query"].as_str().is_some_and(|text| text.starts_with("mutation")))
    .collect();
  let merges = requests
    .iter()
    .filter(|request| request.method == "PUT")
    .map(|request| body(&request.body))
    .collect();
  (graphql, merges)
}

#[test]
fn running_checks_arm_auto_merge_pinned_to_the_head_and_never_merge() {
  let fixture = served(&PrState {
    rollup: pending(),
    ..PrState::default()
  });
  let out = step(&input(&fixture, HEAD));
  assert_eq!(out, json!({"outcome": "waiting", "auto_merge": "armed", "head": "abc123"}));
  let (mutations, merges) = sent(&fixture);
  assert_eq!(mutations.len(), 1);
  assert_eq!(mutations[0]["variables"], json!({"id": "PR_7", "m": "SQUASH", "h": "abc123"}));
  assert!(merges.is_empty(), "{merges:?}");
}

#[test]
fn a_repository_without_auto_merge_waits_for_the_engine_to_merge() {
  let state = PrState {
    rollup: pending(),
    ..PrState::default()
  };
  let fixture = served(&state);
  let refused = r#"{"errors":[{"message":"Auto merge is not allowed for this repository"}]}"#;
  fixture
    .sequence("/graphql", vec![crate::gate_rig::pr_reply(&state), Reply::new(200, refused)])
    .expect("sequence");
  let out = step(&input(&fixture, HEAD));
  assert_eq!(out["outcome"], "waiting");
  assert_eq!(out["auto_merge"], "unavailable");
}

#[test]
fn a_failing_check_or_thread_disarms_and_hands_back_without_a_merge_call() {
  for state in [
    PrState { rollup: failing(), armed: true, ..PrState::default() },
    PrState { unresolved: 1, armed: true, ..PrState::default() },
  ] {
    let fixture = served(&state);
    let out = step(&input(&fixture, HEAD));
    assert_eq!(out["outcome"], "fix", "{out}");
    assert!(out["reasons"].as_array().is_some_and(|reasons| !reasons.is_empty()));
    let (mutations, merges) = sent(&fixture);
    assert!(mutations[0]["query"].as_str().is_some_and(|q| q.contains("disablePullRequestAutoMerge")));
    assert!(merges.is_empty());
  }
}

#[test]
fn a_green_head_merges_pinned_to_its_sha_with_the_preferred_method() {
  let fixture = served(&PrState::default());
  fixture
    .route("/repos/o/r/pulls/7/merge", Reply::new(200, r#"{"merged":true}"#))
    .expect("merge");
  fixture
    .route("/repos/o/r/pulls/7", Reply::new(200, r#"{"merged":true}"#))
    .expect("pull");
  let out = step(&input(&fixture, HEAD));
  assert_eq!(out, json!({"outcome": "merged", "admin_used": false, "head": "abc123"}));
  let (_, merges) = sent(&fixture);
  assert_eq!(merges, [json!({"merge_method": "squash", "sha": "abc123"})]);
}

#[test]
fn a_push_between_gate_and_merge_is_stale_and_never_retried_with_admin() {
  let fixture = served(&PrState::default());
  let moved = r#"{"message":"Head branch was modified. Review and try the merge again."}"#;
  fixture
    .route("/repos/o/r/pulls/7/merge", Reply::new(409, moved))
    .expect("moved");
  let out = step(&input(&fixture, HEAD));
  assert_eq!(out["outcome"], "stale", "{out}");
  assert_eq!(out["admin_used"], false);
  assert_eq!(sent(&fixture).1.len(), 1, "one merge call, no admin retry");
}

#[test]
fn a_protection_only_refusal_merges_with_admin() {
  let fixture = served(&PrState {
    merge_state: "BLOCKED",
    ..PrState::default()
  });
  fixture
    .route("/repos/o/r/pulls/7/merge", Reply::new(200, r#"{"merged":true}"#))
    .expect("merge");
  fixture
    .route("/repos/o/r/pulls/7", Reply::new(200, r#"{"merged":true}"#))
    .expect("pull");
  let out = step(&input(&fixture, HEAD));
  assert_eq!(out["outcome"], "merged");
  assert_eq!(out["admin_used"], true);
}

#[test]
fn a_behind_head_disarms_first_then_waits_for_a_parked_worker() {
  let state = PrState {
    behind: 2,
    armed: true,
    ..PrState::default()
  };
  let fixture = served(&state);
  let out = step(&input(&fixture, Mode::Head { parked: false }));
  assert_eq!(out["outcome"], "busy");
  assert_eq!(out["note"], "worker active");
  assert!(sent(&fixture).0[0]["query"].as_str().is_some_and(|q| q.contains("disable")));
  let fixture = served(&PrState { behind: 2, ..PrState::default() });
  let out = step(&input(&fixture, HEAD));
  assert_eq!(out["outcome"], "handoff");
  assert_eq!(out["note"], "no worktree recorded");
}

#[test]
fn disarm_mode_only_disarms_and_closed_or_merged_prs_are_reported() {
  let fixture = served(&PrState { armed: true, ..PrState::default() });
  assert_eq!(step(&input(&fixture, Mode::Disarm)), json!({"outcome": "disarmed"}));
  assert_eq!(sent(&fixture).0.len(), 1);
  for state in ["MERGED", "CLOSED"] {
    let fixture = served(&PrState { state, ..PrState::default() });
    let out = job(input(&fixture, HEAD))();
    let out: Value = serde_json::from_str(&out).expect("json");
    assert_eq!(out["outcome"], state.to_lowercase());
  }
}

#[test]
fn a_github_failure_is_an_error_outcome_with_the_token_redacted() {
  let fixture = Fixture::start().expect("fixture");
  fixture
    .route("/graphql", Reply::new(500, "boom fixture-token"))
    .expect("graphql");
  let mut job = input(&fixture, HEAD);
  job.env = job.env.with("GH_TOKEN", "fixture-token");
  let out = step(&job);
  assert_eq!(out["outcome"], "error");
  let note = out["note"].as_str().expect("note");
  assert!(!note.contains("fixture-token"), "{note}");
  job.repo = "no-slash".to_owned();
  assert_eq!(step(&job)["note"], "bad repository \"no-slash\"");
}
