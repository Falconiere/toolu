use serde_json::json;
use toolu_runtime::env::Env;

use super::{Fact, apply};
use crate::server::{Engine, Fault};
use crate::source_fix::fixture;

fn finished_job(root: &std::path::Path) -> String {
  use toolu_engine::resources::job_results;

  let mut result =
    job_results::create(root, "a", std::path::Path::new("/work"), "gate").expect("create result");
  result.state = "done".into();
  result.exit_code = Some(0);
  job_results::update(root, &result).expect("finish result");
  result.id
}

fn job_prompts(engine: &Engine) -> usize {
  crate::journal::tail(&engine.paths.journal_dir(), engine.now)
    .expect("journal")
    .iter()
    .filter(|row| row.kind == "prompt" && row.name == "job")
    .count()
}

#[test]
fn finished_job_waits_for_a_pushed_idle_status() {
  let fix = fixture(true).expect("fixture");
  let mut engine = Engine::open(fix.engine_paths, None, Fault::None).expect("open");
  let issue = engine.world.issues.get_mut("a").expect("issue");
  issue.pane = Some("w1:p1".into());
  issue.set_agent_status("working", 1);
  let id = finished_job(fix.tmp.path());
  let env = Env::from_pairs([("HOME", "/tmp")]);
  let event = |status| {
    json!({"event": "pane.agent_status_changed",
    "data": {"pane_id": "w1:p1", "agent_status": status}})
  };
  apply(&mut engine, &env, Fact::Event(event("working"))).expect("working");
  assert_eq!(job_prompts(&engine), 0);
  assert_eq!(
    toolu_engine::resources::job_results::pending(fix.tmp.path(), std::path::Path::new("/work"))
      .expect("pending")[0]
      .id,
    id
  );
  apply(&mut engine, &env, Fact::Event(event("idle"))).expect("idle");
  apply(&mut engine, &env, Fact::Event(event("idle"))).expect("repeat");
  assert_eq!(job_prompts(&engine), 1);
}

#[test]
fn finished_job_appears_on_an_idle_snapshot_once() {
  let fix = fixture(true).expect("fixture");
  let mut engine = Engine::open(fix.engine_paths, None, Fault::None).expect("open");
  engine
    .world
    .issues
    .get_mut("a")
    .expect("issue")
    .set_agent_status("working", 1);
  finished_job(fix.tmp.path());
  let env = Env::from_pairs([("HOME", "/tmp")]);
  let body = json!({"agents": [{"name": "a", "pane_id": "w1:p1", "agent_status": "idle"}]});
  apply(&mut engine, &env, Fact::Snapshot(body.clone())).expect("idle");
  apply(&mut engine, &env, Fact::Snapshot(body)).expect("repeat");
  assert_eq!(job_prompts(&engine), 1);
}

#[test]
fn bad_job_result_does_not_stop_herdr_status_or_later_snapshot_workers() {
  let fix = fixture(true).expect("fixture");
  let mut engine = Engine::open(fix.engine_paths, None, Fault::None).expect("open");
  let issue = engine.world.issues.get_mut("a").expect("issue");
  issue.pane = Some("w1:p1".into());
  issue.set_agent_status("working", 1);
  let epic = fix.tmp.path().join("epic");
  let other =
    crate::logic::ensure_issue(&mut engine.world, "b", "two", &epic.display().to_string());
  other.stage = "running".into();
  other.worktree = Some("/other".into());
  other.set_agent_status("idle", 1);
  std::fs::create_dir_all(fix.tmp.path().join("jobs/results")).expect("result dir");
  std::fs::write(fix.tmp.path().join("jobs/results/bad.json"), "broken").expect("bad result");
  let env = Env::from_pairs([("HOME", "/tmp")]);
  let snapshot = json!({"agents": [
    {"name": "a", "pane_id": "w1:p1", "agent_status": "idle"},
    {"name": "b", "pane_id": "w2:p2", "agent_status": "working"}
  ]});
  apply(&mut engine, &env, Fact::Snapshot(snapshot)).expect("snapshot");
  assert_eq!(engine.world.issues["b"].agent_status, "working");
  let event = json!({"event": "pane.agent_status_changed",
    "data": {"pane_id": "w1:p1", "agent_status": "idle"}});
  apply(&mut engine, &env, Fact::Event(event)).expect("status event");
  let rows = crate::journal::tail(&engine.paths.journal_dir(), engine.now).expect("journal");
  assert_eq!(
    rows
      .iter()
      .filter(|row| row.kind == "action" && row.name == "job-prompt-failed" && row.key == "a")
      .count(),
    2
  );
  assert_eq!(job_prompts(&engine), 0);
}

#[test]
fn a_closed_pane_on_an_idle_issue_does_not_launch_and_its_agent_is_unknown() {
  let fix = fixture(true).expect("fixture");
  let mut engine = Engine::open(fix.engine_paths, None, Fault::None).expect("open");
  let issue = engine.world.issues.get_mut("a").expect("issue");
  issue.stage = "idle".to_owned();
  issue.pane = Some("w1:p1".to_owned());
  issue.set_agent_status("working", 1);
  let line = json!({"event":"pane_closed","data":{"pane_id":"w1:p1"}});
  let env = Env::from_pairs([("HOME", "/tmp")]);
  apply(&mut engine, &env, Fact::Event(line)).expect("apply");
  let issue = &engine.world.issues["a"];
  assert_eq!(issue.launches, 0);
  assert_eq!(issue.agent_status, "unknown");
  assert_ne!(issue.agent_status, "working");
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
  assert_eq!(engine.world.issues["a"].agent_status, "working");
  apply(&mut engine, &env, Fact::Snapshot(json!({"agents": []}))).expect("gone");
  assert_ne!(engine.world.issues["a"].agent_status, "working");
}

fn herdr_events(engine: &Engine) -> usize {
  let page = crate::status_events::page(
    &engine.paths,
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
    assert_eq!(
      herdr_events(&engine),
      before + 1,
      "one journal row per change"
    );
    let issue = &engine.world.issues["a"];
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
    assert_ne!(issue.agent_status, "working");
  }
}
