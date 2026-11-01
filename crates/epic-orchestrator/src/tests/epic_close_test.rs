use serde_json::json;

use super::{arm, commit};
use crate::epics::Epic;
use crate::model::{Action, Issue, Step, World};

fn complete_world() -> World {
  let mut world = World::new(0);
  let mut epic = Epic::new("e", "/state", 0, Some("github"));
  epic.complete = true;
  world.epics.insert("e".into(), epic);
  world
}

fn closing(world: &World) -> bool {
  world.epics["e"]
    .pending
    .as_ref()
    .is_some_and(|pending| pending.action == Action::Close)
}

#[test]
fn a_close_is_armed_only_for_a_complete_open_idle_epic() {
  let mut world = complete_world();
  let mut busy = Issue::blank("a", "e", "/state", 0);
  busy.stage = "cleanup-incomplete".into();
  world.issues.insert("a".into(), busy);
  arm(&mut world, "e");
  assert!(!closing(&world), "an occupied slot keeps the epic open");
  "merged".clone_into(&mut world.issues.get_mut("a").expect("a").stage);
  world.epics.get_mut("e").expect("e").complete = false;
  arm(&mut world, "e");
  assert!(!closing(&world), "an incomplete epic stays open");
  world.epics.get_mut("e").expect("e").complete = true;
  arm(&mut world, "e");
  assert!(closing(&world));
}

#[test]
fn an_applied_close_journals_epic_closed_and_raises_epic_complete_once() {
  let mut world = complete_world();
  arm(&mut world, "e");
  let steps = commit(&mut world, "e", &json!({"outcome": "applied"}));
  assert!(steps.iter().any(
    |step| matches!(step, Step::Journal(row) if row.name == "epic-closed" && row.epic == "e")
  ));
  assert!(world.epics["e"].closed);
  assert_eq!(
    world.epics["e"]
      .pending
      .as_ref()
      .map(|pending| (pending.phase, pending.follow)),
    Some((2, Some(Action::Gc)))
  );
  let done = crate::refresh::commit_journal(
    &mut world,
    &crate::journal::Record::new("action", "close-done", "e", "t", "applied"),
  );
  assert_eq!(done, []);
  assert_eq!(
    world.epics["e"]
      .pending
      .as_ref()
      .map(|pending| (pending.action, pending.phase)),
    Some((Action::Gc, 0)),
    "the sweep follows the close"
  );
  let kinds: Vec<&str> = world
    .attention
    .iter()
    .map(|item| item.kind.as_str())
    .collect();
  assert_eq!(kinds, ["epic-complete"]);
  world.epics.get_mut("e").expect("e").pending = None;
  arm(&mut world, "e");
  assert!(!closing(&world), "a closed epic is never closed again");
}

#[test]
fn an_uncertain_or_failed_close_is_a_judgment_and_never_retried() {
  for (doc, note) in [
    (json!({"outcome": "open"}), "epic close uncertain"),
    (json!({"outcome": "failed", "note": "HTTP 403"}), "HTTP 403"),
  ] {
    let mut world = complete_world();
    arm(&mut world, "e");
    let steps = commit(&mut world, "e", &doc);
    assert!(
      steps
        .iter()
        .any(|step| matches!(step, Step::Journal(row) if row.name == "close-done"))
    );
    assert_eq!(world.attention[0].kind, "failed");
    assert_eq!(world.attention[0].note, note);
    assert!(world.epics["e"].pending.is_none());
    arm(&mut world, "e");
    assert!(!closing(&world));
  }
}
