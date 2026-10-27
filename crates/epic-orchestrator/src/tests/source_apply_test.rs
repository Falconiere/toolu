use serde_json::json;
use toolu_runtime::env::Env;

use super::{Fact, apply};
use crate::server::{Engine, Fault};
use crate::source_fix::fixture;

#[test]
fn a_closed_pane_on_an_idle_issue_does_not_launch() {
  let fix = fixture(true).expect("fixture");
  let mut engine = Engine::open(fix.engine_paths, None, Fault::None).expect("open");
  let issue = engine.world.issues.get_mut("a").expect("issue");
  issue.stage = "idle".to_owned();
  issue.pane = Some("w1:p1".to_owned());
  let line = json!({"event":"pane_closed","data":{"pane_id":"w1:p1"}});
  let env = Env::from_pairs([("HOME", "/tmp")]);
  apply(&mut engine, &env, Fact::Event(line)).expect("apply");
  assert_eq!(engine.world.issues["a"].launches, 0);
}

#[test]
fn a_snapshot_tracks_whether_the_agent_works_and_an_unlisted_one_does_not() {
  let fix = fixture(true).expect("fixture");
  let mut engine = Engine::open(fix.engine_paths, None, Fault::None).expect("open");
  let issue = engine.world.issues.get_mut("a").expect("issue");
  issue.stage = "idle".to_owned();
  let env = Env::from_pairs([("HOME", "/tmp")]);
  let working = json!({"agents": [{"name": "a", "pane_id": "w1:p1", "agent_status": "working"}]});
  apply(&mut engine, &env, Fact::Snapshot(working)).expect("working");
  assert!(engine.world.issues["a"].queue.agent_working);
  apply(&mut engine, &env, Fact::Snapshot(json!({"agents": []}))).expect("gone");
  assert!(!engine.world.issues["a"].queue.agent_working);
}

fn herdr_events(engine: &Engine) -> usize {
  let page = crate::status_events::page(
    &engine.paths.journal_dir(),
    0,
    500,
    &toolu_runtime::config::secrets::Secrets::none(),
  )
  .expect("journal");
  page["events"]
    .as_array()
    .expect("events")
    .iter()
    .filter(|row| row["name"] == "herdr-event")
    .count()
}

#[test]
fn status_events_store_the_word_and_its_time_with_their_journal_row() {
  let fix = fixture(true).expect("fixture");
  let mut engine = Engine::open(fix.engine_paths, None, Fault::None).expect("open");
  let env = Env::from_pairs([("HOME", "/tmp")]);
  let blocked = json!({"agents": [{"name": "a", "pane_id": "w1:p1", "agent_status": "blocked"}]});
  apply(&mut engine, &env, Fact::Snapshot(blocked)).expect("snapshot");
  assert_eq!(engine.world.issues["a"].agent_status, "blocked");
  let mut seen = Vec::new();
  for (at, word) in [(5_000, "working"), (9_000, "idle"), (9_500, "idle")] {
    engine.world.now_ms = at;
    let before = herdr_events(&engine);
    let line = json!({"event": "pane.agent_status_changed",
      "data": {"pane_id": "w1:p1", "agent_status": word}});
    apply(&mut engine, &env, Fact::Event(line)).expect("event");
    assert_eq!(herdr_events(&engine), before + 1, "one journal row per change");
    let issue = &engine.world.issues["a"];
    assert_eq!(issue.queue.agent_working, word == "working");
    seen.push((issue.agent_status.clone(), issue.agent_status_at_ms));
  }
  assert_eq!(
    seen,
    [
      ("working".to_owned(), 5_000),
      ("idle".to_owned(), 9_000),
      ("idle".to_owned(), 9_000)
    ],
    "the time moves only when the word changes"
  );
  let odd = json!({"event": "pane.agent_status_changed",
    "data": {"pane_id": "w1:p1", "agent_status": "thinking"}});
  apply(&mut engine, &env, Fact::Event(odd)).expect("odd word");
  assert_eq!(engine.world.issues["a"].agent_status, "unknown");
  apply(&mut engine, &env, Fact::Snapshot(json!({"agents": []}))).expect("unlisted");
  assert_eq!(engine.world.issues["a"].agent_status, "unknown");
}

#[test]
fn an_outage_marks_every_issue_unknown() {
  let fix = fixture(true).expect("fixture");
  let mut engine = Engine::open(fix.engine_paths, None, Fault::None).expect("open");
  let env = Env::from_pairs([("HOME", "/tmp")]);
  let mut other = engine.world.issues["a"].clone();
  other.key = "b".to_owned();
  other.agent = "b".to_owned();
  engine.world.issues.insert("b".to_owned(), other);
  let rows = json!({"agents": [
    {"name": "a", "pane_id": "w1:p1", "agent_status": "working"},
    {"name": "b", "pane_id": "w1:p2", "agent_status": "idle"}]});
  apply(&mut engine, &env, Fact::Snapshot(rows)).expect("snapshot");
  assert_eq!(engine.world.issues["b"].agent_status, "idle");
  apply(&mut engine, &env, Fact::Outage).expect("outage");
  for issue in engine.world.issues.values() {
    assert_eq!(issue.agent_status, "unknown", "{}", issue.key);
    assert!(!issue.queue.agent_working);
  }
}
