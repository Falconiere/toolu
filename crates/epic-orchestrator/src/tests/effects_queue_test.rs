use serde_json::{Value, json};
use toolu_http_test_support::Fixture;

use super::job;
use crate::gate_rig::{PrState, serve_pr};
use crate::model::{Action, Issue};
use crate::paths::Paths;
use crate::server::{Engine, Fault};
use crate::support::{FAST, reach};

/// An engine whose `o/r#7` issue `k` heads its queue, GitHub on `fixture`.
fn engine(tmp: &tempfile::TempDir, fixture: &Fixture) -> Engine {
  let mut engine = Engine::open(Paths::at(tmp.path().join("engine")), None, Fault::None).expect("open");
  engine.reach = reach(fixture, FAST);
  let state = tmp.path().join("state");
  std::fs::create_dir_all(state.join("issues")).expect("issues");
  let mut issue = Issue::blank("k", "e", &state.display().to_string(), 0);
  "ready".clone_into(&mut issue.phase);
  "running".clone_into(&mut issue.stage);
  "o/r".clone_into(&mut issue.repo);
  issue.pr = Some(7);
  issue.queued_at_ms = Some(1);
  engine.world.issues.insert("k".to_owned(), issue);
  engine
}

fn run(engine: &Engine, action: Action) -> Value {
  let out = job(engine, "k", action)();
  serde_json::from_str(&out).unwrap_or(Value::String(out))
}

#[test]
fn a_working_agent_or_a_herdr_outage_is_never_rebased_under() {
  let tmp = tempfile::tempdir().expect("temp");
  let fixture = Fixture::start().expect("fixture");
  serve_pr(&fixture, &PrState { behind: 1, ..PrState::default() });
  let mut engine = engine(&tmp, &fixture);
  engine.world.issues.get_mut("k").expect("k").agent_working = true;
  assert_eq!(run(&engine, Action::Merge)["note"], "worker active");
  engine.world.issues.get_mut("k").expect("k").agent_working = false;
  engine.world.herdr_failures = 1;
  assert_eq!(run(&engine, Action::Merge)["note"], "worker active");
  engine.world.herdr_failures = 0;
  assert_eq!(run(&engine, Action::Merge)["note"], "no worktree recorded", "parked");
}

#[test]
fn an_issue_out_of_the_queue_only_disarms_and_a_recovered_step_asks_github() {
  let tmp = tempfile::tempdir().expect("temp");
  let fixture = Fixture::start().expect("fixture");
  serve_pr(&fixture, &PrState { armed: true, ..PrState::default() });
  let mut engine = engine(&tmp, &fixture);
  "babysit".clone_into(&mut engine.world.issues.get_mut("k").expect("k").phase);
  assert_eq!(run(&engine, Action::Merge), json!({"outcome": "disarmed"}));
  assert_eq!(run(&engine, Action::Reconcile), json!("open"));
  serve_pr(&fixture, &PrState { state: "MERGED", ..PrState::default() });
  assert_eq!(run(&engine, Action::Reconcile), json!("merged"));
  engine.world.issues.get_mut("k").expect("k").pr = None;
  assert_eq!(run(&engine, Action::Merge), json!("failed: no pull request"));
  assert_eq!(job(&engine, "nobody", Action::Merge)(), "failed: no such issue");
}

#[test]
fn the_recorded_lease_fences_the_worktree() {
  let record = json!({"resource_root": "/r", "lease_token": "t"});
  let lease = super::lease(&record).expect("lease");
  assert_eq!((lease.root.display().to_string(), lease.token), ("/r".to_owned(), "t".to_owned()));
  assert_eq!(super::lease(&json!({"resource_root": "/r", "lease_token": ""})), None);
  assert_eq!(super::lease(&Value::Null), None);
}
