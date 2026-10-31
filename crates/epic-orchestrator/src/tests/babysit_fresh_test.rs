use std::path::{Path, PathBuf};

use toolu_engine::babysit::state_digest;

use super::{Collected, REFRESH_MS, request, reusable};
use crate::model::{Issue, World};
use crate::paths::Paths;
use crate::watch::{CHECK_MS, Kind, Watch};

const KEY: &str = "pr:o/r#1";

/// A babysit issue `one` watching `o/r#1` whose memo saw `state` at time 0.
fn watched(state: &Path) -> World {
  let mut world = World::new(0);
  let mut issue = Issue::blank("one", "epic", "/state", 0);
  issue.phase = "babysit".into();
  issue.stage = "running".into();
  world.issues.insert("one".into(), issue);
  let kind = Kind::Pr {
    key: "one".into(),
    repo: "o/r".into(),
    number: 1,
  };
  let mut watch = Watch::new(kind, 0);
  watch.babysit = std::fs::read(state).ok().map(|bytes| Collected {
    at_ms: 0,
    state_digest: state_digest(&bytes),
    phase_at_ms: 0,
  });
  world.watches.insert(KEY.into(), watch);
  world
}

fn state_file(dir: &Path) -> PathBuf {
  let path = dir.join("o__r-1.json");
  std::fs::write(&path, r#"{"version":2,"repo":"o/r","number":1}"#).expect("state");
  path
}

#[test]
fn a_memo_is_reused_until_the_refresh_bound_and_never_from_the_future() {
  let dir = tempfile::tempdir().expect("temp");
  let state = state_file(dir.path());
  let mut world = watched(&state);
  world.now_ms = REFRESH_MS - CHECK_MS;
  assert!(reusable(&world, KEY, &state).is_some(), "12 minutes old");
  world.now_ms = REFRESH_MS;
  assert!(reusable(&world, KEY, &state).is_none(), "15 minutes old");
  world.now_ms = CHECK_MS;
  if let Some(watch) = world.watches.get_mut(KEY) {
    watch.babysit = watch.babysit.map(|memo| Collected {
      at_ms: 2 * CHECK_MS,
      ..memo
    });
  }
  assert!(
    reusable(&world, KEY, &state).is_none(),
    "a clock that moved back"
  );
}

#[test]
fn ready_a_changed_or_missing_state_file_or_no_memo_collect() {
  let dir = tempfile::tempdir().expect("temp");
  let state = state_file(dir.path());
  let mut world = watched(&state);
  world.now_ms = CHECK_MS;
  assert!(reusable(&world, KEY, &state).is_some());
  world.issues.get_mut("one").expect("issue").phase = "ready".into();
  assert!(reusable(&world, KEY, &state).is_none(), "ready");
  world.issues.get_mut("one").expect("issue").phase = "babysit".into();
  std::fs::write(&state, r#"{"version":2,"repo":"o/r","number":1,"x":1}"#).expect("edit");
  assert!(reusable(&world, KEY, &state).is_none(), "edited state");
  std::fs::remove_file(&state).expect("remove");
  assert!(reusable(&world, KEY, &state).is_none(), "absent state");
  let fresh = watched(&state);
  assert!(fresh.watches[KEY].babysit.is_none(), "no state, no memo");
}

#[test]
fn a_phase_change_or_a_full_last_page_since_the_collection_collects() {
  let dir = tempfile::tempdir().expect("temp");
  let state = state_file(dir.path());
  let mut world = watched(&state);
  world.now_ms = CHECK_MS;
  assert!(reusable(&world, KEY, &state).is_some());
  world.issues.get_mut("one").expect("issue").phase_at_ms = 1;
  assert!(
    reusable(&world, KEY, &state).is_none(),
    "left and re-entered babysit"
  );
  world.issues.get_mut("one").expect("issue").phase_at_ms = 0;
  let tail = "/repos/o/r/issues/1/comments?per_page=100".to_owned();
  let watch = world.watches.get_mut(KEY).expect("watch");
  watch.full_tails.insert(tail);
  assert!(reusable(&world, KEY, &state).is_none(), "a full last page");
}

#[test]
fn only_a_waiting_babysit_or_ready_issue_gets_a_request() {
  let dir = tempfile::tempdir().expect("temp");
  let state = state_file(dir.path());
  let mut world = watched(&state);
  let paths = Paths::at(dir.path());
  let asked = request(&world, KEY, &paths).expect("babysit request");
  assert_eq!(asked.state_file, dir.path().join("babysit/o__r-1.json"));
  assert_eq!(asked.now.as_deref(), Some("1970-01-01T00:00:00Z"));
  world.issues.get_mut("one").expect("issue").stage = "cleaning".into();
  assert!(request(&world, KEY, &paths).is_none());
  world.issues.get_mut("one").expect("issue").stage = "running".into();
  world.issues.get_mut("one").expect("issue").phase = "pr-open".into();
  assert!(request(&world, KEY, &paths).is_none());
  assert!(request(&world, "pr:o/r#2", &paths).is_none());
}
