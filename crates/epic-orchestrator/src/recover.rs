//! Rebuild an in-flight action from the journal tail and the stage files.

use crate::journal::Record;
use crate::logic::{advance_phase, finish_action, token};
use crate::model::{Action, Cycle, Pending, World, fresh};

/// Apply one journal record while rebuilding.
pub(crate) fn observe(world: &mut World, record: &Record) {
  world.seq = world.seq.max(record.seq);
  if record.kind == "judgment" && record.name == "dependency-cycle" {
    world.cycle = Cycle::Sent;
  }
  let Some(action) = action_of(&record.name) else {
    return;
  };
  if record.name.ends_with("-intent") {
    set_recovered(world, record, action);
  } else if record.name.ends_with("-done") {
    let _steps = finish_action(world, &record.key);
  }
}

fn action_of(name: &str) -> Option<Action> {
  let stem = name
    .strip_suffix("-intent")
    .or_else(|| name.strip_suffix("-done"))?;
  match stem {
    "merge" => Some(Action::Merge),
    "cleanup" => Some(Action::Cleanup),
    "launch" => Some(Action::Launch),
    "checkpoint" => Some(Action::Checkpoint),
    _ => None,
  }
}

fn set_recovered(world: &mut World, record: &Record, action: Action) {
  let Some(issue) = world.issues.get_mut(&record.key) else {
    return;
  };
  issue.pending = Some(Pending {
    action,
    token: record.token.clone(),
    phase: 1,
    recovered: true,
    follow: None,
  });
}

/// A merge that already moved the stage only needs its done line.
pub(crate) fn reconcile_stages(world: &mut World) {
  let keys: Vec<String> = world.issues.keys().cloned().collect();
  for key in keys {
    reconcile_one(world, &key);
  }
}

fn reconcile_one(world: &mut World, key: &str) {
  let cleaning = world
    .issues
    .get(key)
    .is_some_and(|issue| issue.stage == "cleaning");
  let merge = world.issues.get(key).is_some_and(merge_pending);
  if cleaning && merge {
    advance_phase(world, key, 2);
    if let Some(pending) = world
      .issues
      .get_mut(key)
      .and_then(|issue| issue.pending.as_mut())
    {
      pending.recovered = false;
    }
  } else if cleaning
    && world
      .issues
      .get(key)
      .is_some_and(|issue| issue.pending.is_none())
  {
    let token = token(world);
    if let Some(issue) = world.issues.get_mut(key) {
      issue.pending = Some(fresh(Action::Cleanup, token));
    }
  }
}

fn merge_pending(issue: &crate::model::Issue) -> bool {
  issue
    .pending
    .as_ref()
    .is_some_and(|pending| pending.action == Action::Merge)
}

#[cfg(test)]
#[path = "tests/recover_test.rs"]
mod tests;
