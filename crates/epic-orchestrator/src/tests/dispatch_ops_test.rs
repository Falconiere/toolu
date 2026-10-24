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
}
