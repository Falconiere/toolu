//! Commit an effect outcome into journal steps.

use crate::journal::Record;
use crate::logic::push_attention;
use crate::model::{Action, Step, World};

/// The effect returned `outcome`.
pub(crate) fn commit_effect(world: &mut World, key: &str, outcome: &str) -> Vec<Step> {
  let action = pending_action(world, key);
  let recovered = pending_recovered(world, key);
  if recovered && outcome == "deferred" {
    return defer(world, key, action);
  }
  if recovered {
    return commit_reconcile(world, key, outcome);
  }
  match outcome {
    "applied" | "already" => applied(world, key, action),
    "hold" => fail_pending(world, key, "coupling-hold", "launch held"),
    "deferred" => defer(world, key, action),
    _ if action == Some(Action::Cleanup) => fail_pending(world, key, "cleanup-incomplete", outcome),
    _ => fail_pending(world, key, "failed", outcome),
  }
}

fn pending_action(world: &World, key: &str) -> Option<Action> {
  world
    .issues
    .get(key)
    .and_then(|issue| issue.pending.as_ref())
    .map(|pending| pending.action)
}

fn pending_recovered(world: &World, key: &str) -> bool {
  world
    .issues
    .get(key)
    .and_then(|issue| issue.pending.as_ref())
    .is_some_and(|pending| pending.recovered)
}

fn applied(world: &mut World, key: &str, action: Option<Action>) -> Vec<Step> {
  if let Some(issue) = world.issues.get_mut(key) {
    if let Some(pending) = issue.pending.as_mut() {
      pending.phase = 2;
    }
    if action == Some(Action::Merge) {
      "cleaning".clone_into(&mut issue.stage);
    }
    if action == Some(Action::Launch) {
      "running".clone_into(&mut issue.stage);
    }
  }
  if action == Some(Action::Merge) {
    crate::watch::after_merge(world, key);
  }
  vec![Step::Issue {
    key: key.to_owned(),
  }]
}

fn defer(world: &mut World, key: &str, action: Option<Action>) -> Vec<Step> {
  if let Some(issue) = world.issues.get_mut(key) {
    issue.deferred = action.map(Action::name).map(str::to_owned);
    issue.pending = None;
  }
  world.outbox.push_back(Step::Journal(Record::new(
    "action",
    "deferred",
    key,
    "",
    action.map_or("", Action::name),
  )));
  world.outbox.drain(..).collect()
}

fn fail_pending(world: &mut World, key: &str, kind: &str, note: &str) -> Vec<Step> {
  if let Some(issue) = world.issues.get_mut(key) {
    issue.pending = None;
  }
  push_attention(world, kind, key, note);
  world.outbox.drain(..).collect()
}

fn commit_reconcile(world: &mut World, key: &str, outcome: &str) -> Vec<Step> {
  match outcome {
    "merged" | "already" => mark_recovered_done(world, key),
    "open" => {
      clear_recovered(world, key);
      Vec::new()
    }
    _ => fail_pending(world, key, "failed", "outcome unknown"),
  }
}

fn mark_recovered_done(world: &mut World, key: &str) -> Vec<Step> {
  if let Some(pending) = world
    .issues
    .get_mut(key)
    .and_then(|issue| issue.pending.as_mut())
  {
    pending.phase = 2;
    pending.recovered = false;
  }
  if let Some(issue) = world.issues.get_mut(key)
    && issue.stage != "merged"
  {
    "cleaning".clone_into(&mut issue.stage);
  }
  crate::watch::after_merge(world, key);
  vec![Step::Issue {
    key: key.to_owned(),
  }]
}

fn clear_recovered(world: &mut World, key: &str) {
  if let Some(pending) = world
    .issues
    .get_mut(key)
    .and_then(|issue| issue.pending.as_mut())
  {
    pending.recovered = false;
  }
}

#[cfg(test)]
#[path = "tests/commit_test.rs"]
mod tests;
