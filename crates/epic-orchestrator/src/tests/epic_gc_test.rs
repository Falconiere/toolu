use serde_json::json;

use super::commit;
use crate::epics::Epic;
use crate::journal::Record;
use crate::model::{Action, Step, World, fresh};

fn world_with_gc() -> World {
  let mut world = World::new(0);
  let mut epic = Epic::new("e", "/state", 0, Some("github"));
  epic.closed = true;
  epic.pending = Some(fresh(Action::Gc, "t".into()));
  world.epics.insert("e".into(), epic);
  world
}

fn swept_note(steps: &[Step]) -> String {
  steps
    .iter()
    .find_map(|step| match step {
      Step::Journal(row) if row.name == "gc-swept" => Some(row.note.clone()),
      Step::Journal(_)
      | Step::Call { .. }
      | Step::Status { .. }
      | Step::Issue { .. }
      | Step::Prompt { .. }
      | Step::Notify { .. } => None,
    })
    .expect("gc-swept")
}

#[test]
fn an_applied_sweep_journals_its_summary_and_ends_the_action() {
  let mut world = world_with_gc();
  let doc = json!({"outcome": "applied", "swept": {"apply": true, "repos": [], "engine": {"job_logs_removed": 3}}});
  let steps = commit(&mut world, "e", &doc);
  assert_eq!(
    swept_note(&steps),
    "worktrees=0 branches=0 wip_refs=0 job_logs=3 sandbox=skipped colima=skipped"
  );
  assert_eq!(world.epics["e"].pending.as_ref().map(|p| p.phase), Some(2));
  assert!(world.attention.is_empty(), "a sweep is never a judgment");
}

#[test]
fn a_failed_sweep_is_one_line_and_blocks_nothing() {
  let mut world = world_with_gc();
  let steps = commit(
    &mut world,
    "e",
    &json!({"outcome": "failed", "note": "git exploded"}),
  );
  assert_eq!(swept_note(&steps), "failed: git exploded");
  assert_eq!(world.epics["e"].pending.as_ref().map(|p| p.phase), Some(2));
  assert!(world.epics["e"].closed && !world.epics["e"].close_blocked);
  let done =
    crate::refresh::commit_journal(&mut world, &Record::new("action", "gc-done", "e", "t", ""));
  assert_eq!(done, []);
  assert!(
    world.epics["e"].pending.is_none(),
    "nothing follows a sweep"
  );
}
