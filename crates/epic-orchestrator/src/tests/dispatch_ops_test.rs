use std::sync::mpsc;

use serde_json::json;

use super::reply_release;
use crate::server::{Engine, Fault};

#[test]
fn release_answers_ok_or_why_not() {
  let fix = crate::source_fix::fixture(true).expect("fixture");
  let mut engine = Engine::open(fix.engine_paths, None, Fault::None).expect("open");
  let (tx, rx) = mpsc::channel();
  reply_release(&mut engine, &json!({"op": "release", "key": "a"}), &tx).expect("not held");
  assert_eq!(rx.recv().expect("reply"), json!({"error": "a is not held"}));
  engine.world.issues.get_mut("a").expect("a").hold =
    Some(json!({"kind": "coupling", "with": "b"}));
  reply_release(&mut engine, &json!({"op": "release", "key": "a"}), &tx).expect("released");
  assert_eq!(
    rx.recv().expect("reply"),
    json!({"ok": true, "key": "a", "released": true})
  );
  let record = crate::disk::read_value(
    &std::path::Path::new(&engine.world.issues["a"].state_dir).join("issues/a.json"),
  )
  .expect("record");
  assert_eq!(
    (record.get("hold"), record["coupling_released"].clone()),
    (None, json!(true))
  );
}

fn finish_step(engine: &mut Engine, request: &serde_json::Value) -> serde_json::Value {
  let (tx, rx) = mpsc::channel();
  super::reply_finish(engine, request, &tx).expect("finish op");
  rx.recv().expect("reply")
}

/// The teardown's own herdr events: the agent's pane exits, then its worktree goes.
fn teardown_events(engine: &mut Engine) {
  let env = toolu_runtime::env::Env::process();
  for line in [
    json!({"event": "pane_exited", "data": {"type": "pane_exited", "pane_id": "w1:p1"}}),
    json!({"event": "worktree_removed", "data": {"type": "worktree_removed", "worktree": {"path": "/work"}}}),
  ] {
    crate::source_apply::apply(engine, &env, crate::source_apply::Fact::Event(line))
      .expect("event");
  }
}

fn finish_engine() -> (crate::source_fix::Fix, Engine) {
  let fix = crate::source_fix::fixture(true).expect("fixture");
  let mut engine = Engine::open(fix.engine_paths.clone(), None, Fault::None).expect("open");
  engine.world.issues.get_mut("a").expect("a").pane = Some("w1:p1".to_owned());
  (fix, engine)
}

#[test]
fn without_finish_begin_the_teardown_events_spend_a_relaunch() {
  let (_fix, mut engine) = finish_engine();
  teardown_events(&mut engine);
  assert_ne!(engine.world.issues["a"].launches, 0);
}

#[test]
fn a_finish_through_the_engine_arms_no_relaunch_and_adopts_the_settled_stage() {
  let (_fix, mut engine) = finish_engine();
  let begin = finish_step(
    &mut engine,
    &json!({"op": "finish", "key": "a", "step": "begin"}),
  );
  assert_eq!(
    begin,
    json!({"ok": true, "key": "a", "tracked": true, "stage": "running"})
  );
  teardown_events(&mut engine);
  let issue = &engine.world.issues["a"];
  assert_eq!(
    (
      issue.stage.as_str(),
      issue.launches,
      issue.pending.is_none()
    ),
    ("cleaning", 0, true)
  );
  let path = crate::snapshot::issue_path(&issue.state_dir, "a");
  crate::disk::write_value(&path, &json!({"stage": "abandoned", "worktree": "/work"}))
    .expect("record");
  let end = finish_step(
    &mut engine,
    &json!({"op": "finish", "key": "a", "step": "end", "stage": "running"}),
  );
  assert_eq!(end, json!({"ok": true, "key": "a", "stage": "abandoned"}));
  assert!(!crate::refill::occupies(&engine.world.issues["a"]));
  let rows = crate::journal::tail(&engine.paths.journal_dir(), engine.now).expect("journal");
  let notes: Vec<&str> = rows
    .iter()
    .filter(|row| row.name == "finish")
    .map(|row| row.note.as_str())
    .collect();
  assert_eq!(notes, ["begin", "abandoned"]);
  assert!(
    !rows
      .iter()
      .any(|row| row.name == "launch-intent" || row.name == "checkpoint-intent")
  );
}

#[test]
fn a_finish_step_is_refused_while_the_engine_merges_and_needs_begin_or_end() {
  let (_fix, mut engine) = finish_engine();
  let unknown = finish_step(
    &mut engine,
    &json!({"op": "finish", "key": "zz", "step": "begin"}),
  );
  assert_eq!(unknown, json!({"ok": true, "key": "zz", "tracked": false}));
  let odd = finish_step(
    &mut engine,
    &json!({"op": "finish", "key": "a", "step": "later"}),
  );
  assert_eq!(odd, json!({"error": "finish needs step begin or end"}));
  engine.world.issues.get_mut("a").expect("a").pending = Some(crate::model::fresh(
    crate::model::Action::Merge,
    "t".to_owned(),
  ));
  let busy = finish_step(
    &mut engine,
    &json!({"op": "finish", "key": "a", "step": "begin"}),
  );
  assert_eq!(
    busy,
    json!({"error": "the engine is merging or cleaning up a; wait for it"})
  );
  assert_eq!(engine.world.issues["a"].stage, "running");
  let fallback = finish_step(
    &mut engine,
    &json!({"op": "finish", "key": "a", "step": "end", "stage": "running"}),
  );
  assert_eq!(fallback["stage"], "running");
  let path = crate::snapshot::issue_path(&engine.world.issues["a"].state_dir, "a");
  crate::disk::write_value(&path, &json!({"stage": null})).expect("record");
  let unrecorded = finish_step(
    &mut engine,
    &json!({"op": "finish", "key": "a", "step": "end", "stage": "uncertain"}),
  );
  assert_eq!(unrecorded["stage"], "uncertain");
}

#[test]
fn begin_drops_a_waiting_relaunch_and_a_checkpoint_s_launch_and_refuses_a_cleanup() {
  let (_fix, mut engine) = finish_engine();
  let issue = engine.world.issues.get_mut("a").expect("a");
  issue.relaunch = Some("lost".to_owned());
  let mut checkpoint = crate::model::fresh(crate::model::Action::Checkpoint, "c".to_owned());
  checkpoint.follow = Some(crate::model::Action::Launch);
  issue.pending = Some(checkpoint);
  let begin = finish_step(
    &mut engine,
    &json!({"op": "finish", "key": "a", "step": "begin"}),
  );
  assert_eq!(begin["tracked"], true);
  crate::schedule::note_event(&mut engine.world, "a", "gone");
  let issue = &engine.world.issues["a"];
  // The checkpoint may already have run; either way it no longer leads to a launch.
  let follow = issue.pending.as_ref().and_then(|pending| pending.follow);
  assert_eq!(
    (issue.relaunch.as_deref(), follow, issue.launches),
    (None, None, 0)
  );
  let rows = crate::journal::tail(&engine.paths.journal_dir(), engine.now).expect("journal");
  assert!(!rows.iter().any(|row| row.name == "launch-intent"));
  engine.world.issues.get_mut("a").expect("a").pending = Some(crate::model::fresh(
    crate::model::Action::Cleanup,
    "t".to_owned(),
  ));
  let busy = finish_step(
    &mut engine,
    &json!({"op": "finish", "key": "a", "step": "begin"}),
  );
  assert_eq!(
    busy,
    json!({"error": "the engine is merging or cleaning up a; wait for it"})
  );
}

#[test]
fn a_cleaning_issue_whose_finish_died_gets_its_live_stage_back_once_no_finish_holds_it() {
  let (_fix, mut engine) = finish_engine();
  let _begin = finish_step(
    &mut engine,
    &json!({"op": "finish", "key": "a", "step": "begin"}),
  );
  engine.flush().expect("flush");
  let state = engine.world.issues["a"].state_dir.clone();
  let lock = std::path::Path::new(&state).join("launch-locks").join("a");
  let held = toolu_engine::resources::lock::acquire_lock(&lock, std::time::Duration::ZERO)
    .expect("lock")
    .expect("free");
  super::heal_cleaning(&mut engine.world, "one");
  assert_eq!(engine.world.issues["a"].stage, "cleaning");
  held.release();
  super::heal_cleaning(&mut engine.world, "one");
  assert_eq!(engine.world.issues["a"].stage, "running");
  let healed = engine.world.outbox.iter().any(|step| {
    matches!(step, crate::model::Step::Journal(row) if row.name == "finish" && row.note == "running")
  });
  assert!(healed);
  let path = crate::snapshot::issue_path(&state, "a");
  crate::disk::write_value(&path, &json!({"stage": "abandoned"})).expect("record");
  engine.world.issues.get_mut("a").expect("a").stage = "cleaning".to_owned();
  super::heal_cleaning(&mut engine.world, "one");
  assert_eq!(engine.world.issues["a"].stage, "cleaning");
}

#[test]
fn pause_and_resume_journal_who_acted_and_on_what() {
  let tmp = tempfile::tempdir().expect("temp");
  let paths = crate::paths::Paths::at(tmp.path());
  let mut engine = Engine::open(paths, None, Fault::None).expect("open");
  for request in [
    json!({"op": "pause"}),
    json!({"op": "resume", "epic": "e-1", "by": "web"}),
  ] {
    let (tx, rx) = mpsc::channel();
    let protocol = crate::PROTOCOL;
    let _stop = crate::dispatch::dispatch(&mut engine, &request, &tx, &mut Vec::new(), protocol);
    assert_eq!(rx.recv().expect("reply"), json!({"ok": true}));
  }
  let rows: Vec<(String, String, String)> =
    crate::journal::tail(&engine.paths.journal_dir(), engine.now)
      .expect("tail")
      .into_iter()
      .filter(|row| row.kind == "action")
      .map(|row| (row.name, row.key, row.note))
      .collect();
  assert_eq!(
    rows,
    [
      ("pause".to_owned(), String::new(), "all by=user".to_owned()),
      (
        "resume".to_owned(),
        "e-1".to_owned(),
        "e-1 by=web".to_owned()
      ),
    ]
  );
}
