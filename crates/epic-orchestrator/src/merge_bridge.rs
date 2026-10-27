//! Between the merge and the cleanup. A merge script that defers raises
//! `merge-ready` with the gate command. A merge the PR detector sees (an
//! armed auto-merge among them), or `toolu epic gate --merge --key K` sends as
//! `event merged`, moves a running issue to `cleaning` and arms its cleanup.

use serde_json::Value;

use crate::logic::{push_attention, token};
use crate::model::{Action, Seen, Step, World, fresh};

/// A deferred merge waits for a human: name the command that merges it.
pub(crate) fn merge_ready(world: &mut World, key: &str) {
  let note = world
    .issues
    .get(key)
    .and_then(|issue| {
      issue
        .pr
        .map(|pr| format!("run toolu epic gate --pr {}#{pr} --merge", issue.repo))
    })
    .unwrap_or_else(|| "run toolu epic gate --pr <repo>#<n> --merge".to_owned());
  push_attention(world, "merge-ready", key, &note);
}

/// The issue's pull request merged: clean it up.
pub(crate) fn merged(world: &mut World, key: &str) {
  let Some(issue) = world.issues.get(key) else {
    return;
  };
  let pending = issue.pending.as_ref().map(|pending| pending.action);
  let running = matches!(issue.stage.as_str(), "running" | "awaiting_merge");
  // A scripted merge in flight arms the cleanup itself when it commits.
  if !running || !matches!(pending, None | Some(Action::Checkpoint)) {
    return;
  }
  let token = token(world);
  if let Some(issue) = world.issues.get_mut(key) {
    "cleaning".clone_into(&mut issue.stage);
    issue.deferred = None;
    issue.relaunch = None;
    match issue.pending.as_mut() {
      Some(checkpoint) => checkpoint.follow = Some(Action::Cleanup),
      None => issue.pending = Some(fresh(Action::Cleanup, token)),
    }
  }
  world.outbox.push_back(Step::Issue {
    key: key.to_owned(),
  });
  unblock_dependents(world, key);
  crate::watch::after_merge(world, key);
}

/// Queue an unblock prompt for every running issue this merge unblocks. A
/// failed delivery falls back to an `unblocked` attention item.
pub(crate) fn unblock_dependents(world: &mut World, key: &str) {
  let waiting: Vec<String> = world
    .issues
    .iter()
    .filter(|(_, issue)| {
      // Only a started worker: a never-launched dependent (empty stage) has
      // no pane, so the prompt would fail and raise a false `unblocked`.
      issue.blockers.iter().any(|blocker| blocker == key)
        && matches!(issue.stage.as_str(), "running" | "awaiting_merge")
    })
    .map(|(dep, _)| dep.clone())
    .collect();
  for dep in waiting {
    let text = format!("unblocked: {key} merged; resume once all your blockers are merged.");
    crate::schedule::enqueue_prompt(world, &dep, &text, "unblocked", Some("unblocked"));
  }
}

/// What the issue record says of the tracker item after the merge.
pub(crate) fn seen_from(record: &Value) -> Seen {
  if record.get("reopened_raised") == Some(&Value::Bool(true)) {
    Seen::Reopened
  } else if record
    .get("tracker_closed_at")
    .is_some_and(|at| !at.is_null())
  {
    Seen::Closed
  } else {
    Seen::Open
  }
}

#[cfg(test)]
#[path = "tests/merge_bridge_test.rs"]
mod tests;
