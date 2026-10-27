//! Stall nudges, reminders, checkpoints, pause, and herdr backoff.

use crate::journal::Record;
use crate::logic::{push_attention, token};
use crate::model::{Action, Pending, QueuedPrompt, Step, World};

/// How long a queued worker prompt waits before it may be sent, so one
/// burst of notices collapses to one prompt.
pub(crate) const PROMPT_DEBOUNCE_MS: u64 = 5_000;

/// Stall nudges and one checkpoint. Steps sit in the outbox or on the issue.
pub(crate) fn on_tick(world: &mut World) {
  let keys: Vec<String> = world.issues.keys().cloned().collect();
  for key in keys {
    stall(world, &key);
  }
  crate::attention::remind(world);
  arm_checkpoint(world);
  crate::refresh::poll(world);
}

fn stall(world: &mut World, key: &str) {
  let Some(due) = stall_due(world, key) else {
    return;
  };
  if due {
    let _token = crate::attention::reraise(world, "stall", key, "stall survived a nudge");
    // One stall judgment per nudge. Ack, or a phase change, starts the next.
    if let Some(issue) = world.issues.get_mut(key) {
      issue.nudge_at_ms = u64::MAX;
    }
    return;
  }
  if let Some(issue) = world.issues.get_mut(key) {
    issue.stall_nudged = true;
    issue.nudge_at_ms = world.now_ms;
  }
  crate::babysit::prompt(world, key, "status", "STATUS?".to_owned(), None);
}

/// Absolute time of the soonest stall deadline.
pub(crate) fn next_stall_ms(world: &World) -> Option<u64> {
  world
    .issues
    .keys()
    .filter_map(|key| {
      let (since, limit, _) = stall_clock(world, key)?;
      Some(since.saturating_add(limit))
    })
    .min()
}

/// `Some(true)` when the nudge already happened, `Some(false)` for the first nudge.
fn stall_due(world: &World, key: &str) -> Option<bool> {
  let (since, limit, nudged) = stall_clock(world, key)?;
  (world.now_ms.saturating_sub(since) >= limit).then_some(nudged)
}

/// `(since, limit, already nudged)` when the phase has a stall deadline.
fn stall_clock(world: &World, key: &str) -> Option<(u64, u64, bool)> {
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
  Some((since, limit, issue.stall_nudged))
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

/// `pane_closed` for a running issue bound to `pane_id` is `gone`.
pub(crate) fn map_pane_closed(world: &mut World, pane_id: &str) {
  let Some(key) = world.issues.values().find_map(|issue| {
    let bound = issue.pane.as_deref() == Some(pane_id) && issue.stage == "running";
    bound.then(|| issue.key.clone())
  }) else {
    return;
  };
  note_event(world, &key, "gone");
}

/// Queue `text` for `key`'s worker. A burst collapses to one prompt: the
/// first notice sets the deadline, later ones add their text, so no notice
/// of the burst is lost.
pub(crate) fn enqueue_prompt(
  world: &mut World,
  key: &str,
  text: &str,
  name: &str,
  fallback: Option<&str>,
) {
  let due_ms = world.now_ms.saturating_add(PROMPT_DEBOUNCE_MS);
  let queued = world
    .prompt_queue
    .entry(key.to_owned())
    .or_insert(QueuedPrompt {
      text: String::new(),
      due_ms,
      name: String::new(),
      fallback: None,
    });
  if queued.text.is_empty() {
    text.clone_into(&mut queued.text);
  } else if !queued.text.contains(text) {
    queued.text.push(' ');
    queued.text.push_str(text);
  }
  name.clone_into(&mut queued.name);
  queued.fallback = fallback.map(str::to_owned);
}

/// Absolute time of the soonest queued-prompt deadline.
pub(crate) fn next_prompt_ms(world: &World) -> Option<u64> {
  world
    .prompt_queue
    .values()
    .map(|queued| queued.due_ms)
    .min()
}

/// Move due queued prompts to the outbox, journaling each delivery like a
/// babysit prompt. Entries for dead agents are dropped. Delivery failures
/// fall back to attention through the prompt's own fallback kind.
pub(crate) fn flush_prompts(world: &mut World) {
  let now = world.now_ms;
  let due: Vec<String> = world
    .prompt_queue
    .iter()
    .filter(|(_, queued)| queued.due_ms <= now)
    .map(|(key, _)| key.clone())
    .collect();
  for key in due {
    let Some(queued) = world.prompt_queue.remove(&key) else {
      continue;
    };
    if crate::attention::has_live_agent(world, &key) {
      let fallback = queued.fallback.as_deref();
      crate::babysit::prompt(world, &key, &queued.name, queued.text, fallback);
    }
  }
}

/// A source event from #446.
pub(crate) fn note_event(world: &mut World, key: &str, event: &str) {
  match event {
    "blocked" => {
      let _token = crate::attention::reraise(world, "blocked", key, "blocked");
    }
    "host-limited" => world.outbox.push_back(Step::Journal(Record::new(
      "action",
      "host-limited",
      key,
      "",
      "",
    ))),
    "gone" => gone(world, key),
    "merged" => crate::merge_bridge::merged(world, key),
    _ => {}
  }
}

fn gone(world: &mut World, key: &str) {
  // A teardown in progress or a settled issue has no agent to relaunch (#619).
  if !crate::attention::has_live_agent(world, key) {
    return;
  }
  let Some(issue) = world.issues.get(key) else {
    return;
  };
  // A relaunch already waiting or in flight owns the loss, and a merge or
  // cleanup no longer needs the agent: neither spends the relaunch budget.
  let pending = issue
    .pending
    .as_ref()
    .map(|pending| (pending.action, pending.follow));
  let checkpointing = pending == Some((Action::Checkpoint, None));
  if issue.relaunch.is_some() || (pending.is_some() && !checkpointing) {
    return;
  }
  if issue.launches >= 2 {
    push_attention(world, "relaunch-limit", key, "relaunch limit");
    return;
  }
  let token = token(world);
  let Some(issue) = world.issues.get_mut(key) else {
    return;
  };
  issue.launches = issue.launches.saturating_add(1);
  match issue.pending.as_mut() {
    // A periodic checkpoint in flight relaunches once it is done.
    Some(checkpoint) => checkpoint.follow = Some(Action::Launch),
    None => {
      issue.pending = Some(Pending {
        action: Action::Checkpoint,
        token,
        phase: 0,
        recovered: false,
        follow: Some(Action::Launch),
        reroute: false,
      });
    }
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

/// Acknowledge the key's attention items and restart its stall clock, so a
/// stall is not raised again until it has run its full length once more.
pub(crate) fn ack(world: &mut World, key: &str, by: &str) {
  crate::attention::ack(world, key, by);
  if let Some(issue) = world.issues.get_mut(key) {
    issue.stall_nudged = false;
    issue.nudge_at_ms = world.now_ms;
    issue.phase_at_ms = world.now_ms;
  }
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

/// Whether `herdr agent list` answers.
pub(crate) fn herdr_ok(session: Option<&str>) -> bool {
  let mut argv = vec!["herdr".to_owned()];
  if let Some(session) = session {
    argv.push("--session".to_owned());
    argv.push(session.to_owned());
  }
  argv.push("agent".to_owned());
  argv.push("list".to_owned());
  let mut spec = toolu_runtime::process::Spec::new(argv);
  spec.timeout = std::time::Duration::from_secs(10);
  toolu_runtime::process::run(&spec).is_ok_and(|output| output.exit_code == 0)
}

#[cfg(test)]
#[path = "tests/schedule_test.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/schedule_gone_test.rs"]
mod gone_tests;

#[cfg(test)]
#[path = "tests/schedule_prompt_test.rs"]
mod prompt_tests;
