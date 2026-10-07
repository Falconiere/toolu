use super::observe;
use crate::journal::Record;
use crate::logic::ensure_issue;
use crate::model::World;

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
