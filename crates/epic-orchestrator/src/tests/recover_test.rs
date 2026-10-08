use super::{observe, reconcile_stages};
use crate::journal::Record;
use crate::logic::ensure_issue;
use crate::model::{Action, Cycle, World, fresh};

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
