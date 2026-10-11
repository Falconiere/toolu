use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use tempfile::TempDir;
use toolu_http_test_support::{Fixture, Reply};
use toolu_runtime::env::Env;

use super::{Request, Source, tick};
use crate::{fixture, recorded};

pub(super) const NOW: &str = "2026-09-19T12:00:00Z";

pub(super) fn request<'a>(state: &'a Path, now: &'a str, number: u64) -> Request<'a> {
  Request {
    repo: "Falconiere/toolu",
    number,
    state_file: state,
    snapshot_out: None,
    now,
    scheduled: false,
  }
}

pub(super) fn recorded_165() -> PathBuf {
  fixture::path("snapshots/toolu-165.json")
}

pub(super) fn read(path: &Path) -> Value {
  serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn sibling(state: &Path, suffix: &str) -> PathBuf {
  crate::state::sibling(state, suffix)
}

#[test]
fn two_ticks_on_a_recorded_snapshot_persist_state_snapshot_and_result() {
  let dir = TempDir::new().unwrap();
  let state = dir.path().join("next.json");
  let snapshot = recorded_165();
  for round in 0..2_u64 {
    let ticked = tick(
      &request(&state, NOW, 165),
      Source::File(&snapshot),
      &Env::default(),
    )
    .unwrap();
    assert_eq!(ticked.result["decision"], "escalate");
    assert_eq!(ticked.result["reasons"][0]["code"], "pr_merged");
    assert_eq!(
      ticked.result["nextCheckAt"],
      Value::Null,
      "an ad-hoc tick is unscheduled"
    );
    assert!(ticked.graphql.is_none());
    let next = read(&state);
    assert_eq!(next["version"], 2);
    assert_eq!(next["totalTicks"], round + 1);
    let snapshot_out = sibling(&state, ".snapshot.json");
    assert_eq!(next["lastGoodSnapshot"], snapshot_out.display().to_string());
    assert_eq!(read(&sibling(&state, ".result.json")), ticked.result);
  }
  assert_eq!(
    std::fs::read(sibling(&state, ".snapshot.json")).unwrap(),
    std::fs::read(&snapshot).unwrap(),
    "the recorded bytes are kept"
  );
  assert!(!dir.path().join("next.json.lock").exists());
}

#[test]
fn scheduled_ticks_report_the_next_whole_slot() {
  let dir = TempDir::new().unwrap();
  let state = dir.path().join("s.json");
  let snapshot = recorded_165();
  let mut first = request(&state, NOW, 165);
  first.scheduled = true;
  let ticked = tick(&first, Source::File(&snapshot), &Env::default()).unwrap();
  assert_eq!(ticked.result["nextCheckAt"], "2026-09-19T12:03:00Z");
  let mut late = request(&state, "2026-09-19T12:04:10Z", 165);
  late.scheduled = true;
  let ticked = tick(&late, Source::File(&snapshot), &Env::default()).unwrap();
  assert_eq!(ticked.result["nextCheckAt"], "2026-09-19T12:06:00Z");
  assert_eq!(read(&state)["schedule"]["nextCheckAt"], 1_789_819_560_u64);
}

#[test]
fn a_snapshot_for_another_pr_or_of_another_shape_is_refused() {
  let dir = TempDir::new().unwrap();
  let state = dir.path().join("s.json");
  let snapshot = recorded_165();
  let other = tick(
    &request(&state, NOW, 166),
    Source::File(&snapshot),
    &Env::default(),
  );
  assert_eq!(other.err().unwrap().code, "slot_mismatch");
  let odd = dir.path().join("odd.json");
  std::fs::write(
    &odd,
    r#"{"version":1,"repo":"Falconiere/toolu","number":165}"#,
  )
  .unwrap();
  let failure = tick(
    &request(&state, NOW, 165),
    Source::File(&odd),
    &Env::default(),
  )
  .err()
  .unwrap();
  assert_eq!(
    (failure.code, failure.extra["source"].as_str()),
    ("invalid_json", Some("snapshot"))
  );
  let missing = dir.path().join("missing.json");
  let failure = tick(
    &request(&state, NOW, 165),
    Source::File(&missing),
    &Env::default(),
  );
  assert_eq!(failure.err().unwrap().code, "invalid_json");
  assert!(!state.exists());
  let bad_now = tick(
    &request(&state, "now", 165),
    Source::File(&snapshot),
    &Env::default(),
  );
  assert_eq!(bad_now.err().unwrap().code, "usage");
}

#[test]
fn a_live_slot_lock_refuses_the_tick_without_touching_state() {
  let dir = TempDir::new().unwrap();
  let state = dir.path().join("s.json");
  let lock = dir.path().join("s.json.lock");
  std::fs::create_dir(&lock).unwrap();
  std::fs::write(lock.join("pid"), format!("{}\n", std::process::id())).unwrap();
  let since = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap();
  std::fs::write(lock.join("since"), format!("{}\n", since.as_secs())).unwrap();
  let snapshot = recorded_165();
  let failure = tick(
    &request(&state, NOW, 165),
    Source::File(&snapshot),
    &Env::default(),
  );
  let failure = failure.err().unwrap();
  assert_eq!(failure.code, "locked");
  assert_eq!(failure.exit(), toolu_protocol::exit::Exit::TempFail);
  assert!(!state.exists());
}

#[test]
fn racing_ticks_count_only_completed_writers() {
  let dir = TempDir::new().unwrap();
  let state = dir.path().join("s.json");
  let snapshot = recorded_165();
  let outcomes: Vec<Result<(), &'static str>> = std::thread::scope(|scope| {
    let runs: Vec<_> = (0..4)
      .map(|_| {
        scope.spawn(|| {
          tick(
            &request(&state, NOW, 165),
            Source::File(&snapshot),
            &Env::default(),
          )
          .map(|_| ())
          .map_err(|failure| failure.code)
        })
      })
      .collect();
    runs.into_iter().map(|run| run.join().unwrap()).collect()
  });
  assert!(
    outcomes
      .iter()
      .all(|outcome| matches!(outcome, Ok(()) | Err("locked")))
  );
  let winners = outcomes.iter().filter(|outcome| outcome.is_ok()).count();
  assert!(winners >= 1);
  assert_eq!(read(&state)["totalTicks"], winners);
  let leftovers = std::fs::read_dir(dir.path()).unwrap().count();
  assert_eq!(leftovers, 3, "state, snapshot and result only");
}

#[test]
fn a_failed_collection_keeps_the_prior_tick_and_stamps_last_error() {
  let dir = TempDir::new().unwrap();
  let state = dir.path().join("s.json");
  let snapshot = recorded_165();
  tick(
    &request(&state, NOW, 165),
    Source::File(&snapshot),
    &Env::default(),
  )
  .unwrap();
  let fixture = Fixture::start().unwrap();
  let busy = json!({"message": "Server Error"}).to_string();
  fixture
    .route(
      &format!("{}/pulls/165", recorded::ROOT),
      Reply::new(502, busy),
    )
    .unwrap();
  let source = Source::GitHub {
    config: recorded::config(&fixture),
    page_size: 100,
  };
  let later = "2026-09-19T12:03:00Z";
  let failure = tick(
    &request(&state, later, 165),
    source,
    &recorded::env(&fixture),
  )
  .err()
  .unwrap();
  assert_eq!(
    (failure.code, failure.extra["class"].as_str()),
    ("api_error", Some("transient"))
  );
  let kept = read(&state);
  assert_eq!(kept["totalTicks"], 1);
  assert_eq!(kept["pr"]["lastError"]["code"], "api_error");
  assert_eq!(kept["pr"]["lastError"]["at"], later);
  assert!(!dir.path().join("s.json.lock").exists());
}

#[test]
fn no_token_is_gh_unavailable() {
  let dir = TempDir::new().unwrap();
  let state = dir.path().join("s.json");
  let path = dir.path().display().to_string();
  let env = Env::from_pairs([("PATH", path.as_str())]);
  let source = Source::GitHub {
    config: toolu_github::Config::scheduled(),
    page_size: 100,
  };
  let failure = tick(&request(&state, NOW, 165), source, &env)
    .err()
    .unwrap();
  assert_eq!(failure.code, "gh_unavailable");
  assert_eq!(failure.exit(), toolu_protocol::exit::Exit::Unavailable);
}

#[test]
fn a_live_tick_collects_reduces_and_reports_its_graphql_cost() {
  let dir = TempDir::new().unwrap();
  let state = dir.path().join("s.json");
  let fixture = Fixture::start().unwrap();
  let sha = recorded::snapshot()["head"]["sha"]
    .as_str()
    .unwrap()
    .to_owned();
  recorded::serve(
    &fixture,
    &[&sha, &sha],
    recorded::replies(&recorded::round()),
  );
  let source = Source::GitHub {
    config: recorded::config(&fixture),
    page_size: 2,
  };
  let ticked = tick(&request(&state, NOW, 115), source, &recorded::env(&fixture)).unwrap();
  assert_eq!(ticked.graphql.map(|usage| usage.points), Some(10));
  let mut written = read(&sibling(&state, ".snapshot.json"));
  let mut expected = recorded::snapshot();
  written.as_object_mut().unwrap().remove("collectedAt");
  expected.as_object_mut().unwrap().remove("collectedAt");
  assert_eq!(written, expected);
  assert_eq!(ticked.result["decision"], "escalate");
  assert_eq!(read(&state)["pr"]["headSha"], sha);
}
