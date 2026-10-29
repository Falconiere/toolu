use std::time::SystemTime;

use super::{UNCERTAIN, observe, reconcile_stages};
use crate::epics::Epic;
use crate::journal::{self, Record};
use crate::logic::{ensure_issue, next};
use crate::model::{Action, Cycle, Step, World, fresh};
use crate::paths::Paths;

/// Issue `a` at `stage` with a recovered `action` intent `t1`.
fn recovered(action: &str, stage: &str) -> World {
  let mut world = World::new(0);
  ensure_issue(&mut world, "a", "e", "/state").stage = stage.to_owned();
  let name = format!("{action}-intent");
  observe(&mut world, &Record::new("action", &name, "a", "t1", ""));
  reconcile_stages(&mut world);
  world
}

/// The first call `next` asks for, after the journal lines ahead of it.
fn first_call(world: &mut World) -> Option<Action> {
  std::iter::from_fn(|| next(world))
    .take(8)
    .find_map(|step| match step {
      Step::Call { action, .. } => Some(action),
      Step::Journal(_)
      | Step::Status { .. }
      | Step::Issue { .. }
      | Step::Prompt { .. }
      | Step::Notify { .. } => None,
    })
}

#[test]
fn an_open_intent_is_recovered() {
  let mut world = World::new(0);
  ensure_issue(&mut world, "a", "one", "/epic");
  observe(
    &mut world,
    &Record::new("action", "merge-intent", "a", "t1", ""),
  );
  let pending = world
    .issues
    .get("a")
    .and_then(|issue| issue.pending.clone())
    .expect("pending");
  assert!(pending.recovered);
  assert_eq!(pending.phase, 1);
  assert_eq!(pending.token, "t1");
}

#[test]
fn a_cycle_and_unknown_names_do_not_invent_an_action() {
  let mut world = World::new(0);
  observe(
    &mut world,
    &Record::new("judgment", "dependency-cycle", "", "", ""),
  );
  assert_eq!(world.cycle, Cycle::Sent);
  ensure_issue(&mut world, "a", "one", "/epic");
  for name in [
    "phase",
    "merge-intent",
    "cleanup-intent",
    "launch-intent",
    "checkpoint-intent",
    "nope-intent",
  ] {
    observe(&mut world, &Record::new("action", name, "missing", "t", ""));
  }
  observe(
    &mut world,
    &Record::new("action", "merge-done", "a", "t1", ""),
  );
}

#[test]
fn cleaning_finishes_a_recovered_merge_or_arms_cleanup() {
  let mut world = World::new(0);
  {
    let issue = ensure_issue(&mut world, "a", "one", "/epic");
    issue.stage = "cleaning".to_owned();
    let mut pending = fresh(Action::Merge, "t2".to_owned());
    pending.recovered = true;
    pending.phase = 1;
    issue.pending = Some(pending);
  }
  reconcile_stages(&mut world);
  let pending = world
    .issues
    .get("a")
    .expect("issue")
    .pending
    .clone()
    .expect("pending");
  assert!(!pending.recovered);
  assert_eq!(pending.phase, 2);
  world.issues.get_mut("a").expect("issue").pending = None;
  reconcile_stages(&mut world);
  let pending = world
    .issues
    .get("a")
    .expect("issue")
    .pending
    .clone()
    .expect("cleanup");
  assert_eq!(pending.action, Action::Cleanup);
}

#[test]
fn a_launch_whose_record_runs_only_needs_its_done_line() {
  for stage in ["running", "awaiting_merge"] {
    let world = recovered("launch", stage);
    let pending = world.issues["a"].pending.clone().expect("pending");
    assert_eq!((pending.phase, pending.recovered), (2, false), "{stage}");
  }
}

#[test]
fn an_uncertain_launch_starts_no_second_agent() {
  for stage in ["starting", "uncertain", "replacing"] {
    let mut world = recovered("launch", stage);
    assert!(world.issues["a"].pending.is_none());
    assert_eq!(world.attention[0].kind, "failed");
    assert_eq!(world.attention[0].note, UNCERTAIN);
    assert!(world.outbox.iter().any(|step| matches!(step, Step::Journal(row) if row.name == "launch-done" && row.note == "uncertain")));
    assert_eq!(first_call(&mut world), None, "{stage}");
  }
}

#[test]
fn a_launch_without_a_record_cleanup_and_checkpoint_run_again() {
  for (action, expected) in [
    ("launch", Action::Launch),
    ("cleanup", Action::Cleanup),
    ("checkpoint", Action::Checkpoint),
  ] {
    let stage = if action == "launch" { "" } else { "running" };
    let mut world = recovered(action, stage);
    let pending = world.issues["a"].pending.clone().expect("pending");
    assert_eq!((pending.phase, pending.recovered), (1, false), "{action}");
    assert_eq!(first_call(&mut world), Some(expected));
  }
}

#[test]
fn a_refresh_runs_again_and_a_close_only_checks_the_tracker() {
  let mut world = World::new(0);
  world
    .epics
    .insert("e".into(), Epic::new("e", "/state", 0, None));
  observe(
    &mut world,
    &Record::new("action", "refresh-intent", "e", "t1", ""),
  );
  reconcile_stages(&mut world);
  assert!(matches!(
    crate::refresh::epic_step(&world),
    Some(Step::Call {
      action: Action::Refresh,
      ..
    })
  ));
  observe(
    &mut world,
    &Record::new("action", "refresh-done", "e", "t1", ""),
  );
  observe(
    &mut world,
    &Record::new("action", "close-intent", "e", "t2", ""),
  );
  reconcile_stages(&mut world);
  let pending = world.epics["e"].pending.clone().expect("close");
  assert!(
    pending.recovered,
    "the close effect checks instead of commenting"
  );
  let _steps = crate::refresh::commit(&mut world, "e", r#"{"outcome":"closed"}"#);
  assert!(world.epics["e"].closed);
  assert_eq!(world.epics["e"].pending.as_ref().map(|p| p.phase), Some(2));
}

#[test]
fn a_closed_or_blocked_epic_is_remembered_across_a_restart() {
  let mut world = World::new(0);
  world
    .epics
    .insert("e".into(), Epic::new("e", "/state", 0, None));
  observe(
    &mut world,
    &Record::new("action", "close-intent", "e", "t1", ""),
  );
  observe(
    &mut world,
    &Record::new("action", "close-done", "e", "t1", "uncertain"),
  );
  assert!(world.epics["e"].close_blocked && world.epics["e"].pending.is_none());
  observe(
    &mut world,
    &Record::new("action", "epic-closed", "e", "", "applied"),
  );
  assert!(world.epics["e"].closed);
}

#[test]
fn an_uncertain_launch_done_line_replays_without_a_stage_change() {
  let mut world = World::new(0);
  ensure_issue(&mut world, "a", "e", "/state");
  observe(
    &mut world,
    &Record::new("action", "launch-intent", "a", "t1", ""),
  );
  observe(
    &mut world,
    &Record::new("action", "launch-done", "a", "t1", "uncertain"),
  );
  assert!(world.issues["a"].pending.is_none());
  assert_eq!(world.issues["a"].stage, "");
}

#[test]
fn a_held_merge_done_line_prevents_a_second_put_after_restart() {
  for outcome in ["uncertain: GitHub answered HTTP 504", "refused: 409"] {
    let mut world = World::new(0);
    let issue = ensure_issue(&mut world, "a", "e", "/state");
    issue.stage = "running".into();
    issue.phase = "ready".into();
    issue.repo = "o/r".into();
    issue.pr = Some(7);
    observe(
      &mut world,
      &Record::new("action", "merge-intent", "a", "t1", ""),
    );
    observe(
      &mut world,
      &Record::new("action", "merge-done", "a", "t1", outcome),
    );
    crate::logic::arm_ready(&mut world);
    assert_eq!(
      world.issues["a"].deferred.as_deref(),
      Some("merge"),
      "{outcome}"
    );
    assert!(world.issues["a"].pending.is_none(), "{outcome}");
    assert_eq!(first_call(&mut world), None, "{outcome}");
  }
}

#[test]
fn a_restart_after_a_held_waiting_or_failed_launch_runs_nothing_again() {
  for note in ["hold", "wait", "failed"] {
    let mut world = World::new(0);
    let issue = ensure_issue(&mut world, "a", "e", "/state");
    if note == "hold" {
      issue.hold = Some(serde_json::json!({"kind": "coupling", "with": "b"}));
    } else {
      issue.deferred = Some("launch".into());
    }
    observe(
      &mut world,
      &Record::new("action", "launch-intent", "a", "t1", ""),
    );
    observe(
      &mut world,
      &Record::new("action", "launch-done", "a", "t1", note),
    );
    reconcile_stages(&mut world);
    assert!(world.issues["a"].pending.is_none(), "{note}");
    assert_eq!(first_call(&mut world), None, "{note}");
  }
}

#[test]
fn a_restart_over_a_starting_record_raises_one_judgment_and_launches_nothing() {
  let tmp = tempfile::tempdir().expect("temp");
  let paths = Paths::at(tmp.path());
  let dir = tmp.path().join("state");
  std::fs::create_dir_all(dir.join("issues")).expect("issues");
  std::fs::write(
    dir.join("graph.json"),
    r#"{"issues":[{"key":"a","open_blockers":[]}]}"#,
  )
  .expect("graph");
  std::fs::write(dir.join("issues").join("a.json"), r#"{"stage":"starting"}"#).expect("record");
  let registry = format!(
    r#"{{"epics":[{{"key":"e","state_dir":"{}"}}]}}"#,
    dir.display()
  );
  std::fs::write(paths.registry(), registry).expect("registry");
  let line = Record::new("action", "launch-intent", "a", "t1", "");
  journal::append(
    &paths.journal_dir(),
    &paths.journal_lock(),
    &line,
    SystemTime::now(),
  )
  .expect("append");
  let mut world = crate::snapshot::load(&paths, SystemTime::now()).expect("load");
  assert_eq!(world.attention.len(), 1);
  assert_eq!(first_call(&mut world), None);
}
