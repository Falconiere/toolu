//! Lost agents: `gone` relaunches twice, then raises `relaunch-limit`, and
//! spends nothing while an action or a waiting relaunch owns the issue.

use super::note_event;
use crate::logic::ensure_issue;
use crate::model::{Action, World};

#[test]
fn events_record_blocked_gone_and_host_limits() {
  let mut world = World::new(0);
  ensure_issue(&mut world, "a", "one", "/epic");
  note_event(&mut world, "a", "blocked");
  assert!(world.attention.iter().any(|item| item.kind == "blocked"));
  note_event(&mut world, "a", "host-limited");
  assert!(!world.outbox.is_empty());
  let before = world.outbox.len();
  note_event(&mut world, "a", "nope");
  assert_eq!(world.outbox.len(), before);
  note_event(&mut world, "a", "gone");
  // The relaunch in flight owns the issue: a second miss spends nothing.
  note_event(&mut world, "a", "gone");
  assert_eq!(world.issues["a"].launches, 1);
  world.issues.get_mut("a").expect("a").pending = None;
  world.issues.get_mut("a").expect("a").relaunch = Some("lost".into());
  note_event(&mut world, "a", "gone");
  assert_eq!(world.issues["a"].launches, 1);
  world.issues.get_mut("a").expect("a").relaunch = None;
  note_event(&mut world, "a", "gone");
  assert_eq!(world.issues.get("a").expect("issue").launches, 2);
  world.issues.get_mut("a").expect("a").pending = None;
  note_event(&mut world, "a", "gone");
  assert!(
    world
      .attention
      .iter()
      .any(|item| item.kind == "relaunch-limit")
  );
}

#[test]
fn a_gone_worker_relaunches_twice_then_raises_the_relaunch_limit() {
  let mut world = World::new(0);
  let mut issue = crate::model::Issue::blank("a", "one", "/s", 0);
  issue.stage = "running".to_owned();
  world.issues.insert("a".into(), issue);
  for expected in [1, 2] {
    super::note_event(&mut world, "a", "gone");
    let pending = world
      .issues
      .get_mut("a")
      .and_then(|issue| issue.pending.take())
      .expect("relaunch");
    assert_eq!(
      (pending.action, pending.follow, world.issues["a"].launches),
      (Action::Checkpoint, Some(Action::Launch), expected)
    );
  }
  super::note_event(&mut world, "a", "gone");
  assert!(world.issues["a"].pending.is_none());
  assert!(
    world
      .attention
      .iter()
      .any(|item| item.kind == "relaunch-limit")
  );
}

#[test]
fn a_loss_during_a_checkpoint_relaunches_after_it_and_a_merge_spends_nothing() {
  let mut world = World::new(0);
  let issue = ensure_issue(&mut world, "a", "one", "/epic");
  issue.stage = "running".into();
  issue.pending = Some(crate::model::fresh(Action::Checkpoint, "c".into()));
  note_event(&mut world, "a", "gone");
  let pending = world.issues["a"].pending.clone().expect("checkpoint");
  assert_eq!(
    (pending.action, pending.follow, world.issues["a"].launches),
    (Action::Checkpoint, Some(Action::Launch), 1)
  );
  world.issues.get_mut("a").expect("a").pending =
    Some(crate::model::fresh(Action::Merge, "m".into()));
  note_event(&mut world, "a", "gone");
  assert_eq!(world.issues["a"].launches, 1);
}
