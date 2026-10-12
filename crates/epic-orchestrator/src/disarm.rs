//! Disarm the GitHub auto-merge the engine armed on an issue that left the
//! merge queue (#448), whatever took it out: a worker report, a babysit
//! finding, a hand-off. The disarm is a `merge` step in disarm mode; a failed
//! one is tried again after the fixed check interval, and its third failure
//! in a row is a `failed` judgment.

use crate::logic::{paused, push_attention, token};
use crate::model::{Action, Step, World, fresh};
use crate::queue::queued;
use crate::watch::CHECK_MS;

/// The failure that raises a judgment.
const FAILURES: u32 = 3;

/// Arm the disarm step of every issue that is due one.
pub(crate) fn arm(world: &mut World) {
  let due: Vec<String> = world
    .issues
    .values()
    .filter(|issue| {
      issue.queue.disarm_due
        && issue.pending.is_none()
        && !queued(issue)
        && world.now_ms >= issue.queue.disarm_at_ms
        && !paused(world, issue)
    })
    .map(|issue| issue.key.clone())
    .collect();
  for key in due {
    let token = token(world);
    if let Some(issue) = world.issues.get_mut(&key) {
      issue.pending = Some(fresh(Action::Merge, token));
    }
  }
}

/// Whether `key`'s step was a disarm.
pub(crate) fn disarming(world: &World, key: &str) -> bool {
  world
    .issues
    .get(key)
    .is_some_and(|issue| issue.queue.disarm_due && !queued(issue))
}

/// A disarm step ended: done, or failed with `note`.
pub(crate) fn committed(world: &mut World, key: &str, failure: Option<&str>) {
  let now = world.now_ms;
  let Some(issue) = world.issues.get_mut(key) else {
    return;
  };
  let queue = &mut issue.queue;
  let Some(note) = failure else {
    queue.auto_armed = false;
    queue.disarm_due = false;
    queue.disarm_errors = 0;
    world.outbox.push_back(Step::Issue {
      key: key.to_owned(),
    });
    return;
  };
  queue.disarm_at_ms = now.saturating_add(CHECK_MS);
  queue.disarm_errors = queue.disarm_errors.saturating_add(1);
  if queue.disarm_errors == FAILURES {
    let text = format!("auto-merge disarm keeps failing: {note}");
    push_attention(world, "failed", key, &text);
  }
}

#[cfg(test)]
#[path = "tests/disarm_test.rs"]
mod tests;
