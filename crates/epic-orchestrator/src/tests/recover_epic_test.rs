//! Epic-level records across a restart: a blocked close, `epic-closed`, and the
//! sweep that follows a close.

use super::observe;
use crate::epics::Epic;
use crate::journal::Record;
use crate::model::{Action, World};

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
  observe(
    &mut world,
    &Record::new("action", "gc-intent", "e", "t2", ""),
  );
  assert_eq!(
    world.epics["e"]
      .pending
      .as_ref()
      .map(|pending| (pending.action, pending.recovered)),
    Some((Action::Gc, true)),
    "an interrupted sweep runs again"
  );
  observe(&mut world, &Record::new("action", "gc-done", "e", "t2", ""));
  assert!(world.epics["e"].pending.is_none());
}
