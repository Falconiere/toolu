//! Rebuild an in-flight action from the journal tail and the stage files.
//! After a restart an intent without its done line is decided per action:
//! a launch by its record's stage (never a second agent for an uncertain
//! one), cleanup, checkpoint and refresh run again, a close only asks the
//! tracker, and a merge reconciles through its script.

use crate::journal::Record;
use crate::logic::{advance_phase, finish_action, push_attention, token};
use crate::model::{Action, Cycle, Pending, Step, World, fresh};

/// What a human runs after a launch that may have started an agent.
const UNCERTAIN: &str =
  "uncertain launch: inspect, then `toolu epic launch --reprompt` or `--replace`";

/// Apply one journal record while rebuilding.
pub(crate) fn observe(world: &mut World, record: &Record) {
  world.seq = world.seq.max(record.seq);
  if record.kind == "judgment" && record.name == "dependency-cycle" {
    world.cycle = Cycle::Sent;
  }
  crate::attention::observe(world, record);
  observe_epic(world, record);
  let Some(action) = action_of(&record.name) else {
    return;
  };
  if matches!(action, Action::Refresh | Action::Close) {
    return;
  }
  if record.name.ends_with("-intent") {
    set_recovered(world, record, action);
  } else if record.name.ends_with("-done") && !record.note.is_empty() {
    // A launch that ended without a stage change: held, waiting, failed or
    // uncertain.
    if let Some(issue) = world.issues.get_mut(&record.key) {
      issue.pending = None;
    }
  } else if record.name.ends_with("-done") {
    let _steps = finish_action(world, &record.key);
  }
}

/// Epic-level records: their actions, `epic-closed` and a blocked close.
fn observe_epic(world: &mut World, record: &Record) {
  let Some(epic) = world.epics.get_mut(&record.key) else {
    return;
  };
  match record.name.as_str() {
    "epic-closed" => epic.closed = true,
    "refresh-intent" | "close-intent" => {
      let action = action_of(&record.name).unwrap_or(Action::Refresh);
      epic.pending = Some(recovered(action, &record.token));
    }
    "refresh-done" | "close-done" => {
      epic.pending = None;
      if matches!(record.note.as_str(), "failed" | "uncertain") {
        epic.close_blocked = true;
      }
    }
    _ => {}
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
    "refresh" => Some(Action::Refresh),
    "close" => Some(Action::Close),
    _ => None,
  }
}

fn recovered(action: Action, token: &str) -> Pending {
  Pending {
    action,
    token: token.to_owned(),
    phase: 1,
    recovered: true,
    follow: None,
    reroute: false,
  }
}

fn set_recovered(world: &mut World, record: &Record, action: Action) {
  let Some(issue) = world.issues.get_mut(&record.key) else {
    return;
  };
  issue.pending = Some(recovered(action, &record.token));
}

/// Decide every recovered action once the stage files are overlaid.
pub(crate) fn reconcile_stages(world: &mut World) {
  let keys: Vec<String> = world.issues.keys().cloned().collect();
  for key in keys {
    resume_action(world, &key);
    reconcile_one(world, &key);
  }
  let now = world.now_ms;
  for epic in world.epics.values_mut() {
    if let Some(pending) = epic.pending.as_mut()
      && pending.action == Action::Refresh
    {
      pending.recovered = false;
      epic.refresh_at_ms = now;
    }
  }
}

/// A recovered launch, cleanup or checkpoint; a merge keeps reconciling.
fn resume_action(world: &mut World, key: &str) {
  let Some((action, stage, token)) = world.issues.get(key).and_then(|issue| {
    let pending = issue.pending.as_ref().filter(|pending| pending.recovered)?;
    Some((pending.action, issue.stage.clone(), pending.token.clone()))
  }) else {
    return;
  };
  match action {
    Action::Launch if matches!(stage.as_str(), "starting" | "uncertain" | "replacing") => {
      uncertain(world, key, &token);
    }
    // No record: the launch never started, so it runs again.
    Action::Launch if stage.is_empty() => resume(world, key, 1),
    Action::Launch => resume(world, key, 2),
    Action::Cleanup | Action::Checkpoint | Action::Refresh => resume(world, key, 1),
    Action::Merge | Action::Reconcile | Action::Prompt | Action::Close => {}
  }
}

fn resume(world: &mut World, key: &str, phase: u8) {
  if let Some(pending) = world
    .issues
    .get_mut(key)
    .and_then(|issue| issue.pending.as_mut())
  {
    pending.phase = phase;
    pending.recovered = false;
  }
}

/// No second agent: a human inspects; the done line ends the intent.
fn uncertain(world: &mut World, key: &str, token: &str) {
  if let Some(issue) = world.issues.get_mut(key) {
    issue.pending = None;
  }
  push_attention(world, "failed", key, UNCERTAIN);
  let line = Record::new("action", "launch-done", key, token, "uncertain");
  world.outbox.push_back(Step::Journal(line));
}

/// A merge that already moved the stage only needs its done line.
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
