use std::path::Path;
use std::time::{Instant, SystemTime};

use serde_json::{Value, json};
use toolu_http_test_support::{Fixture, Reply};
use toolu_runtime::env::Env;

use super::{arm, commit, commit_journal, epic_step, poll};
use crate::effects_epic::{EpicJob, job};
use crate::epics::Epic;
use crate::model::{Action, Seen, Step, World};
use crate::paths::Paths;
use crate::retry::Policy;

const MINUTE: u64 = 60_000;

fn world_with(epic: Epic) -> World {
  let mut world = World::new(1_000_000);
  world.epics.insert(epic.key.clone(), epic);
  world
}

/// Write the intent line's effect: the call that follows it.
fn to_call(world: &mut World, epic: &str) -> Step {
  let Some(Step::Journal(intent)) = epic_step(world) else {
    panic!("no intent");
  };
  assert_eq!(
    (intent.name.as_str(), intent.epic.as_str()),
    ("refresh-intent", epic)
  );
  let _none = commit_journal(world, &intent);
  epic_step(world).expect("call")
}

/// A state directory whose graph has `issues` as `(key, state)`.
fn state_dir(root: &Path, issues: &[(&str, &str)]) -> String {
  let rows: Vec<Value> = issues
    .iter()
    .map(|(key, state)| {
      json!({"ref": format!("o/r#{key}"), "title": key, "state": state, "url": "", "repo": "o/r",
        "number": 1, "key": key, "status": "done"})
    })
    .collect();
  let dir = root.join("state");
  std::fs::create_dir_all(dir.join("issues")).expect("issues");
  std::fs::create_dir_all(dir.join("status")).expect("status");
  let graph = json!({"epic": {"ref": "o/r#100", "title": "E", "state": "open", "url": ""},
    "state_dir": dir.display().to_string(), "max_parallel": 2, "issues": rows});
  std::fs::write(dir.join("graph.json"), graph.to_string()).expect("graph");
  dir.display().to_string()
}

fn record(dir: &str, key: &str, body: &Value) {
  let path = Path::new(dir).join("issues").join(format!("{key}.json"));
  std::fs::write(path, body.to_string()).expect("record");
}

fn registered(root: &Path, dir: &str) -> World {
  let registry = json!({"version": 1, "epics": [{"key": "e", "state_dir": dir}]});
  std::fs::write(root.join("registry.json"), registry.to_string()).expect("registry");
  crate::snapshot::load(&Paths::at(root), SystemTime::now()).expect("load")
}

fn judgments(world: &World, kind: &str) -> usize {
  world
    .attention
    .iter()
    .filter(|item| item.kind == kind)
    .count()
}

#[test]
fn a_jira_epic_refreshes_every_15_minutes_and_a_github_one_never_polls() {
  let mut world = world_with(Epic::new("j", "/state", 1_000_000, Some("jira")));
  world
    .epics
    .insert("g".into(), Epic::new("g", "/g", 1_000_000, Some("github")));
  world.now_ms = 1_000_000 + 15 * MINUTE - 1;
  poll(&mut world);
  assert!(world.epics["j"].pending.is_none());
  world.now_ms += 1;
  poll(&mut world);
  assert_eq!(
    world.epics["j"]
      .pending
      .as_ref()
      .map(|pending| pending.action),
    Some(Action::Refresh)
  );
  assert!(world.epics["g"].pending.is_none());
  let Step::Call { key, action, .. } = to_call(&mut world, "j") else {
    panic!("no call");
  };
  assert_eq!((key.as_str(), action), ("j", Action::Refresh));
}

#[test]
fn a_failed_refresh_keeps_its_graph_and_succeeds_180_s_later() {
  let tmp = tempfile::tempdir().expect("temp");
  let dir = state_dir(tmp.path(), &[("a", "open")]);
  let mut world = world_with(Epic::new("e", &dir, 1_000_000, Some("github")));
  arm(&mut world, "e");
  let _call = to_call(&mut world, "e");
  let steps = commit(&mut world, "e", r#"{"outcome":"failed","note":"HTTP 502"}"#);
  assert!(steps.iter().any(|step| matches!(step, Step::Journal(row) if row.name == "refresh-error" && row.note == "HTTP 502")));
  assert_eq!(world.epics["e"].refresh_failures, 1);
  assert!(world.epics["e"].graph.is_none(), "the old graph is kept");
  world.now_ms += 179_999;
  assert!(epic_step(&world).is_none());
  world.now_ms += 1;
  assert!(matches!(
    epic_step(&world),
    Some(Step::Call {
      action: Action::Refresh,
      ..
    })
  ));
  let _steps = commit(&mut world, "e", r#"{"outcome":"applied"}"#);
  assert_eq!(world.epics["e"].refresh_failures, 0);
  assert_eq!(world.epics["e"].refresh_at_ms, u64::MAX);
  assert!(
    world.issues.contains_key("a"),
    "the reload adds the graph's issues"
  );
  let Some(Step::Journal(done)) = epic_step(&world) else {
    panic!("no done line");
  };
  assert_eq!(done.name, "refresh-done");
  let _steps = commit_journal(&mut world, &done);
  assert!(world.epics["e"].pending.is_none());
}

#[test]
fn three_failures_in_a_row_raise_one_refresh_failed() {
  let mut world = world_with(Epic::new("e", "/state", 1_000_000, Some("github")));
  arm(&mut world, "e");
  let _call = to_call(&mut world, "e");
  for _ in 0..4 {
    let _steps = commit(
      &mut world,
      "e",
      r#"{"outcome":"failed","note":"JIRA_PAT is not set"}"#,
    );
    world.now_ms += 180_000;
  }
  assert_eq!(judgments(&world, "refresh-failed"), 1);
  let item = world
    .attention
    .iter()
    .find(|item| item.kind == "refresh-failed")
    .expect("judgment");
  assert_eq!((item.key.as_str(), item.epic.as_str()), ("e", "e"));
  assert_eq!(world.epics["e"].refresh_failures, 4);
}

#[test]
fn a_reopened_merged_issue_raises_one_judgment_across_a_restart_and_does_not_launch() {
  let tmp = tempfile::tempdir().expect("temp");
  let dir = state_dir(tmp.path(), &[("a", "open")]);
  record(
    &dir,
    "a",
    &json!({"stage": "merged", "tracker_closed_at": "2026-10-11T00:00:00.000Z"}),
  );
  let mut world = registered(tmp.path(), &dir);
  assert_eq!(world.issues["a"].tracker_seen, Seen::Closed);
  arm(&mut world, "e");
  let _call = to_call(&mut world, "e");
  let steps = commit(&mut world, "e", r#"{"outcome":"applied"}"#);
  assert_eq!(judgments(&world, "reopened"), 1);
  assert!(
    steps
      .iter()
      .any(|step| matches!(step, Step::Issue { key } if key == "a"))
  );
  let path = crate::snapshot::issue_path(&dir, "a");
  crate::disk::write_tracker_seen(&path, Seen::Reopened, SystemTime::now()).expect("write");
  let mut world = registered(tmp.path(), &dir);
  assert_eq!(world.issues["a"].tracker_seen, Seen::Reopened);
  arm(&mut world, "e");
  let _call = to_call(&mut world, "e");
  let _steps = commit(&mut world, "e", r#"{"outcome":"applied"}"#);
  assert_eq!(judgments(&world, "reopened"), 0);
  crate::refill::refill(&mut world);
  assert_eq!(crate::refill_rig::armed(&world).len(), 0);
}

#[test]
fn a_merged_issue_seen_closed_is_noted_and_an_abandoned_open_one_raises_nothing() {
  let tmp = tempfile::tempdir().expect("temp");
  let dir = state_dir(tmp.path(), &[("a", "closed"), ("b", "open")]);
  record(&dir, "a", &json!({"stage": "merged"}));
  record(&dir, "b", &json!({"stage": "abandoned"}));
  let mut world = registered(tmp.path(), &dir);
  arm(&mut world, "e");
  let _call = to_call(&mut world, "e");
  let _steps = commit(&mut world, "e", r#"{"outcome":"applied"}"#);
  assert_eq!(world.issues["a"].tracker_seen, Seen::Closed);
  assert_eq!(world.issues["b"].tracker_seen, Seen::Open);
  assert_eq!(world.attention.len(), 0);
  crate::refill::refill(&mut world);
  assert_eq!(crate::refill_rig::armed(&world).len(), 0);
}

#[test]
fn a_rate_limited_refresh_spends_its_retry_budget_then_waits_the_fixed_deadline() {
  let fixture = Fixture::start().expect("fixture");
  fixture
    .route(
      "/repos/O/R/issues/1",
      Reply::new(429, r#"{"message":"secondary rate limit"}"#).header("Retry-After", "2"),
    )
    .expect("route");
  let env = Env::from_pairs([("EPIC_API_MAX_SLEEP_S", "3"), ("EPIC_API_BACKOFF_MS", "1")]);
  let tmp = tempfile::tempdir().expect("temp");
  let started = Instant::now();
  let outcome = job(
    Action::Refresh,
    EpicJob {
      state_dir: tmp.path().to_path_buf(),
      reference: "O/R#1".into(),
      tracker: Some("github".into()),
      default_repo: String::new(),
      max_parallel: 2,
      recovered: false,
      reach: crate::support::reach(&fixture, Policy::from_env(&env)),
      env,
      paths: Paths::at(tmp.path()),
    },
  )();
  let elapsed = started.elapsed().as_millis();
  assert_eq!(fixture.requests().expect("requests").len(), 2);
  assert!((2_900..4_500).contains(&elapsed), "{elapsed}");
  assert!(outcome.contains("the sleep budget is spent"), "{outcome}");
  let mut world = world_with(Epic::new("e", "/state", 1_000_000, Some("github")));
  arm(&mut world, "e");
  let _call = to_call(&mut world, "e");
  let _steps = commit(&mut world, "e", &outcome);
  assert_eq!(world.epics["e"].refresh_at_ms, 1_000_000 + 180_000);
  assert!(!std::fs::exists(tmp.path().join("graph.json")).expect("exists"));
}
