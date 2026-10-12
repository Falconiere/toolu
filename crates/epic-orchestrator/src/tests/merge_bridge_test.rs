use serde_json::json;

use super::{merge_ready, merged, seen_from};
use crate::logic::ensure_issue;
use crate::model::{Action, Seen, Step, World, fresh};

fn running(world: &mut World) {
  let issue = ensure_issue(world, "a", "e", "/state");
  issue.stage = "running".into();
  issue.repo = "o/r".into();
  issue.pr = Some(12);
}

#[test]
fn a_deferred_merge_names_the_gate_command() {
  let mut world = World::new(0);
  running(&mut world);
  merge_ready(&mut world, "a");
  assert_eq!(world.attention[0].kind, "merge-ready");
  assert_eq!(
    world.attention[0].note,
    "run toolu epic gate --pr o/r#12 --merge"
  );
}

#[test]
fn a_merge_seen_or_sent_moves_a_running_issue_to_cleaning_and_arms_cleanup() {
  let mut world = World::new(0);
  running(&mut world);
  world.issues.get_mut("a").expect("a").deferred = Some("merge".into());
  crate::schedule::note_event(&mut world, "a", "merged");
  let issue = &world.issues["a"];
  assert_eq!(issue.stage, "cleaning");
  assert_eq!(issue.deferred, None);
  assert_eq!(
    issue.pending.as_ref().map(|p| p.action),
    Some(Action::Cleanup)
  );
  assert!(
    world
      .outbox
      .iter()
      .any(|step| matches!(step, Step::Issue { key } if key == "a"))
  );
  let token = issue.pending.as_ref().map(|p| p.token.clone());
  merged(&mut world, "a");
  assert_eq!(
    world.issues["a"].pending.as_ref().map(|p| p.token.clone()),
    token
  );
}

#[test]
fn a_checkpoint_finishes_first_and_a_scripted_merge_arms_its_own_cleanup() {
  let mut world = World::new(0);
  running(&mut world);
  world.issues.get_mut("a").expect("a").pending = Some(fresh(Action::Checkpoint, "t1".into()));
  merged(&mut world, "a");
  let pending = world.issues["a"].pending.clone().expect("checkpoint");
  assert_eq!(
    (pending.action, pending.follow),
    (Action::Checkpoint, Some(Action::Cleanup))
  );
  let mut world = World::new(0);
  running(&mut world);
  world.issues.get_mut("a").expect("a").pending = Some(fresh(Action::Merge, "t1".into()));
  merged(&mut world, "a");
  assert_eq!(world.issues["a"].stage, "running");
  ensure_issue(&mut world, "b", "e", "/state");
  merged(&mut world, "b");
  assert!(
    world.issues["b"].pending.is_none(),
    "never launched: nothing to clean"
  );
}

#[test]
fn the_record_says_what_was_seen_of_the_tracker_item() {
  assert_eq!(seen_from(&json!({})), Seen::Open);
  assert_eq!(seen_from(&json!({"tracker_closed_at": null})), Seen::Open);
  assert_eq!(seen_from(&json!({"tracker_closed_at": "t"})), Seen::Closed);
  assert_eq!(
    seen_from(&json!({"tracker_closed_at": "t", "reopened_raised": true})),
    Seen::Reopened
  );
}
