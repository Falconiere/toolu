use super::note_herdr;
use crate::model::World;

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
  let issue = crate::logic::ensure_issue(&mut world, "a", "one", "/epic");
  issue.phase = "running".to_owned();
  issue.stall_nudged = true;
  super::ack(&mut world, "a");
  assert!(!world.issues.get("a").expect("issue").stall_nudged);
}
