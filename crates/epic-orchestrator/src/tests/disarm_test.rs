use super::{arm, committed, disarming};
use crate::model::{Action, Issue, World};
use crate::queue_fields::note_phase;

/// `k` was queued with auto-merge armed; its worker reported `phase`.
fn world(phase: &str) -> World {
  let mut world = World::new(1_000);
  let mut issue = Issue::blank("k", "e", "/state", 0);
  "running".clone_into(&mut issue.stage);
  "o/r".clone_into(&mut issue.repo);
  issue.pr = Some(7);
  note_phase(&mut issue, "ready", 0);
  issue.queue.auto_armed = true;
  phase.clone_into(&mut issue.phase);
  note_phase(&mut issue, phase, 500);
  world.issues.insert("k".to_owned(), issue);
  world
}

fn pending(world: &World) -> Option<Action> {
  world.issues["k"]
    .pending
    .as_ref()
    .map(|pending| pending.action)
}

#[test]
fn leaving_the_queue_with_auto_merge_armed_arms_one_disarm_step() {
  let mut world = world("implementing");
  assert!(world.issues["k"].queue.disarm_due);
  assert!(disarming(&world, "k"));
  arm(&mut world);
  assert_eq!(pending(&world), Some(Action::Merge));
  let token = world.issues["k"].pending.clone();
  arm(&mut world);
  assert_eq!(world.issues["k"].pending, token, "never a second step");
  world.issues.get_mut("k").expect("k").pending = None;
  committed(&mut world, "k", None);
  let queue = &world.issues["k"].queue;
  assert_eq!((queue.auto_armed, queue.disarm_due), (false, false));
  arm(&mut world);
  assert_eq!(pending(&world), None);
}

#[test]
fn a_ready_issue_or_a_paused_epic_is_not_disarmed() {
  let mut world = world("ready");
  assert!(!world.issues["k"].queue.disarm_due, "the head step owns it");
  arm(&mut world);
  assert_eq!(pending(&world), None);
  let mut world = self::world("babysit");
  world.paused.insert("e".to_owned());
  arm(&mut world);
  assert_eq!(pending(&world), None);
}

#[test]
fn a_failed_disarm_waits_a_check_interval_and_the_third_is_a_judgment() {
  let mut world = world("babysit");
  for attempt in 1..=3 {
    arm(&mut world);
    assert_eq!(pending(&world), Some(Action::Merge), "attempt {attempt}");
    world.issues.get_mut("k").expect("k").pending = None;
    committed(&mut world, "k", Some("502"));
    arm(&mut world);
    assert_eq!(
      pending(&world),
      None,
      "waits before attempt {}",
      attempt + 1
    );
    world.now_ms += crate::watch::CHECK_MS;
  }
  assert_eq!(world.issues["k"].queue.disarm_errors, 3);
  assert_eq!(world.attention.len(), 1);
  assert_eq!(world.attention[0].kind, "failed");
  assert!(
    world.attention[0]
      .note
      .contains("auto-merge disarm keeps failing: 502")
  );
  committed(&mut world, "nobody", None);
}
