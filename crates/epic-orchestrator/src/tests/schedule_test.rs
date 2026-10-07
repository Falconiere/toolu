use super::{ack, note_event, note_herdr, on_tick, set_pause, take_judgment};
use crate::logic::ensure_issue;
use crate::model::{Action, Attention, World, fresh};

#[test]
fn herdr_backoff_doubles_from_thirty_seconds() {
  let mut world = World::new(5_000);
  note_herdr(&mut world, false);
  assert_eq!(world.herdr_failures, 1);
  assert_eq!(world.herdr_retry_at_ms, 35_000);
  note_herdr(&mut world, false);
  assert_eq!(world.herdr_failures, 2);
  assert_eq!(world.herdr_retry_at_ms, 65_000);
  note_herdr(&mut world, true);
  assert_eq!(world.herdr_failures, 0);
  assert_eq!(world.herdr_retry_at_ms, 0);
}

#[test]
fn ack_clears_a_stall_nudge() {
  let mut world = World::new(10);
  let issue = ensure_issue(&mut world, "a", "one", "/epic");
  issue.phase = "running".to_owned();
  issue.stall_nudged = true;
  world.attention.push(stall("a"));
  ack(&mut world, "a");
  assert!(!world.issues.get("a").expect("issue").stall_nudged);
  assert!(world.attention[0].delivered);
}

#[test]
fn a_stall_nudges_once_then_becomes_a_judgment() {
  let mut world = World::new(0);
  world.stall_ms = 10;
  {
    let issue = ensure_issue(&mut world, "a", "one", "/epic");
    issue.phase = "execution".to_owned();
    issue.phase_at_ms = 0;
  }
  world.now_ms = 10;
  on_tick(&mut world);
  assert!(world.issues.get("a").expect("issue").stall_nudged);
  assert!(!world.outbox.is_empty());
  world.outbox.clear();
  world.now_ms = 20;
  on_tick(&mut world);
  assert!(world.attention.iter().any(|item| item.kind == "stall"));
  let raised = world.attention.len();
  world.now_ms = 10_000;
  on_tick(&mut world);
  assert_eq!(world.attention.len(), raised);
}

#[test]
fn quiet_phases_do_not_stall() {
  let mut world = World::new(100);
  world.stall_ms = 1;
  for phase in ["", "ready", "needs-human", "failed", "blocked"] {
    {
      let issue = ensure_issue(&mut world, "a", "one", "/epic");
      issue.phase = phase.to_owned();
      issue.phase_at_ms = 0;
    }
    on_tick(&mut world);
  }
  assert_eq!(world.outbox.len(), 0);
  assert_eq!(world.attention.len(), 0);
}

#[test]
fn babysit_uses_its_own_stall_limit() {
  let mut world = World::new(10);
  world.stall_ms = 1;
  world.babysit_stall_ms = 100;
  {
    let issue = ensure_issue(&mut world, "a", "one", "/epic");
    issue.phase = "babysit".to_owned();
    issue.phase_at_ms = 0;
  }
  on_tick(&mut world);
  assert!(!world.issues.get("a").expect("issue").stall_nudged);
  world.now_ms = 100;
  on_tick(&mut world);
  assert!(world.issues.get("a").expect("issue").stall_nudged);
}

#[test]
fn checkpoint_waits_then_arms_one_running_worktree() {
  let mut world = World::new(0);
  world.checkpoint_at_ms = 0;
  {
    let issue = ensure_issue(&mut world, "a", "one", "/epic");
    issue.pending = Some(fresh(Action::Merge, "t".to_owned()));
  }
  on_tick(&mut world);
  assert!(world.issues.get("a").expect("issue").pending.is_some());
  {
    let issue = world.issues.get_mut("a").expect("issue");
    issue.pending = None;
    issue.stage = "idle".to_owned();
  }
  on_tick(&mut world);
  assert_eq!(world.checkpointed.len(), 0);
  world.checkpoint_at_ms = 0;
  {
    let issue = world.issues.get_mut("a").expect("issue");
    issue.stage = "running".to_owned();
    issue.worktree = Some("/work".to_owned());
  }
  on_tick(&mut world);
  assert!(world.checkpointed.contains("/work"));
  let pending = world.issues.get("a").expect("issue").pending.clone();
  assert_eq!(pending.expect("checkpoint").action, Action::Checkpoint);
}

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
  note_event(&mut world, "a", "gone");
  assert_eq!(world.issues.get("a").expect("issue").launches, 2);
  note_event(&mut world, "a", "gone");
  assert!(
    world
      .attention
      .iter()
      .any(|item| item.kind == "relaunch-limit")
  );
}

#[test]
fn pause_applies_to_every_epic_or_one() {
  let mut world = World::new(0);
  set_pause(&mut world, None, true);
  assert!(world.paused_all);
  set_pause(&mut world, Some("one"), true);
  assert!(world.paused.contains("one"));
  set_pause(&mut world, Some("one"), false);
  assert!(!world.paused.contains("one"));
  set_pause(&mut world, None, false);
  assert!(!world.paused_all);
  assert!(take_judgment(&mut world).is_none());
  note_event(&mut world, "a", "blocked");
  assert_eq!(take_judgment(&mut world).expect("item").kind, "blocked");
  assert!(take_judgment(&mut world).is_none());
}

fn stall(key: &str) -> Attention {
  Attention {
    seq: 1,
    kind: "stall".to_owned(),
    key: key.to_owned(),
    epic: "one".to_owned(),
    note: String::new(),
    delivered: false,
  }
}
