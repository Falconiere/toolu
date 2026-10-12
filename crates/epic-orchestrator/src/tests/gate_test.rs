use serde_json::{Value, json};
use toolu_http_test_support::{Fixture, Reply};
use toolu_runtime::env::Env;

use super::{GateOpts, run_gate};
use crate::gate_rig::{PrState, serve_pr, settle, trackers};
use crate::support::gh;

fn opts(state: &std::path::Path) -> GateOpts {
  GateOpts {
    pr: "o/r#7".into(),
    state_dir: Some(state.to_path_buf()),
    key: Some("r-5".into()),
    ..GateOpts::default()
  }
}

fn record(state: &std::path::Path) -> Value {
  serde_json::from_str(&std::fs::read_to_string(state.join("issues/r-5.json")).expect("record"))
    .expect("json")
}

/// A green PR whose merge succeeds, issue 5 closed by it, and epic 1.
fn serve_merge(fixture: &Fixture) {
  serve_pr(fixture, &PrState::default());
  let reply = |body: Value| Reply::new(200, body.to_string());
  fixture
    .route("/repos/o/r/pulls/7/merge", reply(json!({"merged": true})))
    .expect("merge");
  fixture
    .route("/repos/o/r/pulls/7", reply(json!({"merged": true})))
    .expect("state");
  fixture
    .route(
      "/repos/o/r/issues/5",
      reply(json!({"state": "closed", "title": "Ship"})),
    )
    .expect("issue");
  fixture
    .route(
      "/repos/o/r/issues/1",
      reply(json!({"body": "- [ ] https://github.com/o/r/issues/5\n"})),
    )
    .expect("epic");
}

#[test]
fn a_merge_verdict_merges_settles_and_records_the_gate() {
  let (tmp, fixture) = (
    tempfile::tempdir().expect("temp"),
    Fixture::start().expect("fixture"),
  );
  serve_merge(&fixture);
  let (env, client, make) = (
    Env::from_pairs([("HOME", "/tmp")]),
    gh(&fixture),
    trackers(&fixture),
  );
  let at = settle(&env, &client, make.as_ref());
  let mut flags = opts(tmp.path());
  flags.merge = true;
  flags.issue = Some("o/r#5".into());
  flags.epic = Some("o/r#1".into());
  let result = run_gate(&flags, &at).expect("gate");
  assert_eq!(
    (
      result["verdict"].clone(),
      result["merged"].clone(),
      result["method"].clone()
    ),
    (json!("merge"), json!(true), json!("squash"))
  );
  assert_eq!(
    (result["issue"].clone(), result["epic_ticked"].clone()),
    (json!("closed-by-pr"), json!(true))
  );
  let saved = record(tmp.path());
  assert_eq!(saved["stage"], "running");
  assert_eq!(saved["last_gate"]["verdict"], "merge");
  assert_eq!(saved["last_gate"]["merged"], true);
  assert_eq!(saved["last_gate"]["auto_merge"], "off");
}

/// Serve `state`, run the gate with `edit`'s flags; the state dir and the
/// result.
fn gate_on(state: &PrState, edit: impl FnOnce(&mut GateOpts)) -> (tempfile::TempDir, Value) {
  let (tmp, _fixture, result) = gate_over(state, |_| {}, edit);
  (tmp, result)
}

/// [`gate_on`] after `prepare` sets up the state dir; the fixture too.
fn gate_over(
  state: &PrState,
  prepare: impl FnOnce(&std::path::Path),
  edit: impl FnOnce(&mut GateOpts),
) -> (tempfile::TempDir, Fixture, Value) {
  let (tmp, fixture) = (
    tempfile::tempdir().expect("temp"),
    Fixture::start().expect("fixture"),
  );
  std::fs::create_dir_all(tmp.path().join("issues")).expect("issues");
  prepare(tmp.path());
  serve_pr(&fixture, state);
  let mut flags = opts(tmp.path());
  edit(&mut flags);
  let result = {
    let (env, client, make) = (
      Env::from_pairs([("HOME", "/tmp")]),
      gh(&fixture),
      trackers(&fixture),
    );
    run_gate(&flags, &settle(&env, &client, make.as_ref())).expect("gate")
  };
  (tmp, fixture, result)
}

#[test]
fn a_waiting_pr_arms_auto_merge_and_records_awaiting_merge() {
  let state = PrState {
    rollup: json!([{"__typename": "CheckRun", "name": "ci", "status": "IN_PROGRESS"}]),
    ..PrState::default()
  };
  let (tmp, fixture, result) = gate_over(&state, |_| {}, |flags| flags.auto = true);
  assert_eq!(result["verdict"], "wait");
  assert_eq!(result["auto_merge"], true);
  let sent: Vec<Value> = fixture
    .requests()
    .expect("requests")
    .iter()
    .filter_map(|request| serde_json::from_slice::<Value>(&request.body).ok())
    .filter(|body| {
      body["query"]
        .as_str()
        .is_some_and(|q| q.contains("enablePullRequestAutoMerge"))
    })
    .map(|body| body["variables"].clone())
    .collect();
  assert_eq!(sent, [json!({"id": "PR_7", "m": "SQUASH", "h": "abc123"})]);
  let saved = record(tmp.path());
  assert_eq!(
    (
      saved["stage"].clone(),
      saved["last_gate"]["reasons"].clone()
    ),
    (json!("awaiting_merge"), json!(["pending: ci"]))
  );
  assert_eq!(saved["last_gate"]["auto_merge"], "armed");
}

#[test]
fn a_gate_keeps_a_stage_the_engine_already_moved() {
  let state = PrState {
    rollup: json!([{"__typename": "CheckRun", "name": "ci", "status": "IN_PROGRESS"}]),
    ..PrState::default()
  };
  for (stage, after) in [
    ("cleaning", "cleaning"),
    ("merged", "merged"),
    ("running", "awaiting_merge"),
  ] {
    let before = json!({"stage": stage, "kind": "claude"});
    let write = |dir: &std::path::Path| {
      std::fs::write(dir.join("issues/r-5.json"), before.to_string()).expect("record");
    };
    let (tmp, _fixture, result) = gate_over(&state, write, |_| {});
    assert_eq!(result["verdict"], "wait");
    let saved = record(tmp.path());
    assert_eq!(saved["stage"], after);
    assert_eq!(saved["kind"], "claude");
    assert_eq!(saved["last_gate"]["verdict"], "wait");
  }
}

#[test]
fn a_fix_verdict_disarms_armed_auto_merge() {
  let state = PrState {
    unresolved: 1,
    armed: true,
    ..PrState::default()
  };
  let (tmp, result) = gate_on(&state, |_| {});
  assert_eq!(
    (
      result["verdict"].clone(),
      result["auto_merge_disarmed"].clone()
    ),
    (json!("fix"), json!(true))
  );
  assert_eq!(record(tmp.path())["last_gate"]["auto_merge"], "off");
}

#[test]
fn a_dry_run_assesses_and_writes_nothing() {
  let (tmp, fixture) = (
    tempfile::tempdir().expect("temp"),
    Fixture::start().expect("fixture"),
  );
  serve_pr(&fixture, &PrState::default());
  let (env, client, make) = (
    Env::from_pairs([("HOME", "/tmp")]),
    gh(&fixture),
    trackers(&fixture),
  );
  let at = settle(&env, &client, make.as_ref());
  let mut flags = opts(tmp.path());
  flags.merge = true;
  flags.dry_run = true;
  let result = run_gate(&flags, &at).expect("gate");
  assert_eq!(
    (result["verdict"].clone(), result["dry_run"].clone()),
    (json!("merge"), json!(true))
  );
  assert!(result.get("merged").is_none() && !tmp.path().join("issues").exists());
  assert!(
    fixture
      .requests()
      .expect("requests")
      .iter()
      .all(|request| request.method == "GET" || request.path == "/graphql")
  );
  assert!(
    run_gate(
      &GateOpts {
        pr: "nonsense".into(),
        ..GateOpts::default()
      },
      &at
    )
    .is_err()
  );
}

#[test]
fn an_unwritable_record_is_reported_with_the_result_and_a_held_lock_refuses() {
  let unwritable = |dir: &std::path::Path| {
    std::fs::create_dir_all(dir.join("issues/r-5.json")).expect("a directory in the way");
  };
  let (_tmp, _fixture, result) = gate_over(&PrState::default(), unwritable, |_| {});
  assert_eq!(result["verdict"], "merge");
  assert!(result["record_error"].is_string(), "{result}");
  let tmp = tempfile::tempdir().expect("temp");
  let path = tmp.path().join("launch-locks/r-5");
  let lock = toolu_engine::resources::lock::acquire_lock(&path, std::time::Duration::ZERO)
    .expect("lock")
    .expect("free");
  let fields = serde_json::Map::new();
  let held = super::record_gate(
    tmp.path(),
    "r-5",
    &fields,
    "wait",
    std::time::Duration::ZERO,
  );
  assert_eq!(
    held,
    Err("r-5: a launch or finish holds its record; gate again".to_owned())
  );
  assert!(!tmp.path().join("issues/r-5.json").exists());
  lock.release();
  super::record_gate(
    tmp.path(),
    "r-5",
    &fields,
    "wait",
    std::time::Duration::ZERO,
  )
  .expect("free");
  assert_eq!(record(tmp.path())["stage"], "awaiting_merge");
}
