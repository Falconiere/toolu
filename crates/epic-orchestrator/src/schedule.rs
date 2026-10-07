//! Stall nudges, checkpoints, pause, and herdr backoff.

use crate::journal::Record;
use crate::logic::{push_attention, token};
use crate::model::{Action, Attention, Pending, Step, World};

/// Stall nudges and one checkpoint. Steps sit in the outbox or on the issue.
pub(crate) fn on_tick(world: &mut World) {
  let keys: Vec<String> = world.issues.keys().cloned().collect();
  for key in keys {
    stall(world, &key);
  }
  arm_checkpoint(world);
}

fn stall(world: &mut World, key: &str) {
  let Some(due) = stall_due(world, key) else {
    return;
  };
  if due {
    push_attention(world, "stall", key, "stall survived a nudge");
    return;
  }
  if let Some(issue) = world.issues.get_mut(key) {
    issue.stall_nudged = true;
    issue.nudge_at_ms = world.now_ms;
  }
  world.outbox.push_back(Step::Journal(Record::new(
    "prompt", "status", key, "", "STATUS?",
  )));
  world.outbox.push_back(Step::Call {
    key: key.to_owned(),
    action: Action::Prompt,
    token: String::new(),
  });
}

/// `Some(true)` when the nudge already happened, `Some(false)` for the first nudge.
fn stall_due(world: &World, key: &str) -> Option<bool> {
  let issue = world.issues.get(key)?;
  if issue.phase.is_empty() || issue.phase == "ready" || judgment(&issue.phase) {
    return None;
  }
  let limit = if issue.phase == "babysit" {
    world.babysit_stall_ms
  } else {
    world.stall_ms
  };
  let since = if issue.stall_nudged {
    issue.nudge_at_ms
  } else {
    issue.phase_at_ms
  };
  (world.now_ms.saturating_sub(since) >= limit).then_some(issue.stall_nudged)
}

fn judgment(phase: &str) -> bool {
  matches!(phase, "needs-human" | "failed" | "blocked")
}

fn arm_checkpoint(world: &mut World) {
  if world.now_ms < world.checkpoint_at_ms
    || world.issues.values().any(|issue| issue.pending.is_some())
  {
    return;
  }
  let Some(key) = next_checkpoint(world) else {
    world.checkpoint_at_ms = world.now_ms.saturating_add(world.checkpoint_every_ms);
    world.checkpointed.clear();
    return;
  };
  let tree = world
    .issues
    .get(&key)
    .and_then(|issue| issue.worktree.clone());
  if let Some(tree) = tree {
    world.checkpointed.insert(tree);
  }
  let token = token(world);
  if let Some(issue) = world.issues.get_mut(&key) {
    issue.pending = Some(crate::model::fresh(Action::Checkpoint, token));
  }
}

fn next_checkpoint(world: &World) -> Option<String> {
  world.issues.values().find_map(|issue| {
    let tree = issue.worktree.as_deref().unwrap_or("");
    let due = issue.stage == "running" && !tree.is_empty() && !world.checkpointed.contains(tree);
    due.then(|| issue.key.clone())
  })
}

/// A source event from #446.
pub(crate) fn note_event(world: &mut World, key: &str, event: &str) {
  match event {
    "blocked" => push_attention(world, "blocked", key, "blocked"),
    "host-limited" => world.outbox.push_back(Step::Journal(Record::new(
      "action",
      "host-limited",
      key,
      "",
      "",
    ))),
    "gone" => gone(world, key),
    _ => {}
  }
}

fn gone(world: &mut World, key: &str) {
  let launches = world.issues.get(key).map_or(0, |issue| issue.launches);
  if launches >= 2 {
    push_attention(world, "relaunch-limit", key, "relaunch limit");
    return;
  }
  let token = token(world);
  if let Some(issue) = world.issues.get_mut(key) {
    issue.launches = issue.launches.saturating_add(1);
    issue.pending = Some(Pending {
      action: Action::Checkpoint,
      token,
      phase: 0,
      recovered: false,
      follow: Some(Action::Launch),
    });
  }
}

/// Pause or resume every epic, or one epic when `epic` is set.
pub(crate) fn set_pause(world: &mut World, epic: Option<&str>, paused_on: bool) {
  match epic {
    None => world.paused_all = paused_on,
    Some(epic) if paused_on => {
      world.paused.insert(epic.to_owned());
    }
    Some(epic) => {
      world.paused.remove(epic);
    }
  }
}

/// Acknowledge a stall so `wait` does not return it again yet.
pub(crate) fn ack(world: &mut World, key: &str) {
  for item in &mut world.attention {
    if item.key == key && item.kind == "stall" {
      item.delivered = true;
    }
  }
  if let Some(issue) = world.issues.get_mut(key) {
    issue.stall_nudged = false;
    issue.nudge_at_ms = world.now_ms;
    issue.phase_at_ms = world.now_ms;
  }
}

/// The oldest undelivered judgment.
pub(crate) fn take_judgment(world: &mut World) -> Option<Attention> {
  let item = world.attention.iter_mut().find(|item| !item.delivered)?;
  item.delivered = true;
  Some(item.clone())
}

/// Herdr probe result. Backoff is 30 seconds, doubling, capped at 5 minutes.
pub(crate) fn note_herdr(world: &mut World, ok: bool) {
  if ok {
    world.herdr_failures = 0;
    world.herdr_retry_at_ms = 0;
    return;
  }
  world.herdr_failures = world.herdr_failures.saturating_add(1);
  let exp = world.herdr_failures.saturating_sub(1).min(10);
  let delay = 30_000u64.saturating_mul(1_u64 << exp).min(300_000);
  world.herdr_retry_at_ms = world.now_ms.saturating_add(delay);
}

#[cfg(test)]
#[path = "tests/schedule_test.rs"]
mod tests;
