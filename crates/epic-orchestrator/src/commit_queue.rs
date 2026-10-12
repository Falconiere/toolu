//! Commit the merge-queue head step (#448): its outcome document becomes
//! engine state, journal lines and, only on a conflict, a `REBASE` prompt.
//! A merged head goes the way of every merge (stage `cleaning`, then
//! cleanup); an engine rebase, a wait or a refusal to touch the worker keep
//! it at the head; a conflict or a failing check takes it out of the queue,
//! and the next head is armed.

use serde_json::{Value, json};

use crate::journal::Record;
use crate::logic::{arm_ready, push_attention};
use crate::model::{Action, Step, World};
use crate::queue_fields::{By, count_round, note_phase};

/// Commit `doc`, the head step's outcome, or `None` when it is not one.
pub(crate) fn commit(world: &mut World, key: &str, doc: &str) -> Option<Vec<Step>> {
  let doc: Value = serde_json::from_str(doc).ok()?;
  let outcome = doc.get("outcome").and_then(Value::as_str)?.to_owned();
  let text = |name: &str| {
    doc
      .get(name)
      .and_then(Value::as_str)
      .unwrap_or("")
      .to_owned()
  };
  let note = text("note");
  if outcome == "merged" {
    return Some(crate::commit::applied(world, key, Some(Action::Merge)));
  }
  close(world, key, &outcome, &note);
  match outcome.as_str() {
    "rebased" => {
      round(world, key, By::Engine, &text("new_head"));
      check_now(world, key);
    }
    "stale" => check_now(world, key),
    "waiting" => set_armed(world, key, text("auto_merge") == "armed"),
    "disarmed" => crate::disarm::committed(world, key, None),
    "error" if crate::disarm::disarming(world, key) => {
      crate::disarm::committed(world, key, Some(&note));
    }
    "busy" => journal(world, "action", "rebase-skipped", key, &note),
    "handoff" => handoff(world, key, &note),
    "fix" => {
      set_armed(world, key, false);
      let reasons = doc.get("reasons").map(Value::to_string).unwrap_or_default();
      leave(world, key, "babysit", &reasons);
    }
    "closed" => closed(world, key),
    "uncertain" | "refused" => held(world, key, &outcome, &note),
    _ => {}
  }
  release_held(world, key);
  Some(world.outbox.drain(..).collect())
}

/// A conflict (or an unsafe worktree): the worker rebases, prompted once.
/// A babysit finding held during the step rides along with `REBASE`, so it
/// is neither lost nor sent as a second, competing prompt.
fn handoff(world: &mut World, key: &str, note: &str) {
  set_armed(world, key, false);
  round(world, key, By::Worker, "");
  leave(world, key, "rebasing", note);
  let held = world
    .issues
    .get_mut(key)
    .and_then(|issue| issue.queue.held.take());
  let mut text = "REBASE".to_owned();
  if let Some(held) = held {
    if let Some(attention) = &held.note {
      push_attention(world, "babysit", key, attention);
    }
    text = format!(
      "REBASE\n\nAfter the rebase, babysit also reported:\n{}",
      held.prompt
    );
  }
  crate::babysit::prompt(world, key, "rebase", text, Some("rebase"));
}

/// The PR was closed without a merge: the queue moves on.
fn closed(world: &mut World, key: &str) {
  if let Some(issue) = world.issues.get_mut(key) {
    "closed".clone_into(&mut issue.stage);
  }
  world.outbox.push_back(Step::Issue {
    key: key.to_owned(),
  });
  push_attention(world, "pr-closed", key, "pull request closed");
  arm_ready(world);
}

/// A merge that may have happened, or was refused for another reason, is
/// never resent: the head is held for a human or the PR detector.
fn held(world: &mut World, key: &str, outcome: &str, note: &str) {
  if let Some(issue) = world.issues.get_mut(key) {
    issue.deferred = Some("merge".to_owned());
  }
  push_attention(world, "failed", key, &format!("merge {outcome}: {note}"));
}

/// End the pending step without a stage change; its done line carries the
/// outcome, so a restart neither runs it again nor finishes it as a merge.
fn close(world: &mut World, key: &str, outcome: &str, note: &str) {
  let Some(pending) = world
    .issues
    .get_mut(key)
    .and_then(|issue| issue.pending.take())
  else {
    return;
  };
  let done = if note.is_empty() {
    outcome.to_owned()
  } else {
    format!("{outcome}: {note}")
  };
  world.outbox.push_back(Step::Journal(Record::new(
    "action",
    "merge-done",
    key,
    &pending.token,
    &done,
  )));
}

/// Count a rebase round and journal who performed it.
fn round(world: &mut World, key: &str, by: By, head: &str) {
  let Some(issue) = world.issues.get_mut(key) else {
    return;
  };
  let round = count_round(issue, by);
  let note = if head.is_empty() {
    json!({"by": by.name(), "round": round})
  } else {
    json!({"by": by.name(), "head": head, "round": round})
  };
  journal(world, "action", "rebase", key, &note.to_string());
  world.outbox.push_back(Step::Issue {
    key: key.to_owned(),
  });
}

/// Record whether auto-merge is armed; armed on an issue that already left
/// the queue, it is due a disarm.
fn set_armed(world: &mut World, key: &str, armed: bool) {
  let Some(issue) = world.issues.get_mut(key) else {
    return;
  };
  let before = (issue.queue.auto_armed, issue.queue.disarm_due);
  issue.queue.auto_armed = armed;
  if armed && !crate::queue::queued(issue) {
    issue.queue.disarm_due = true;
  }
  if before != (issue.queue.auto_armed, issue.queue.disarm_due) {
    world.outbox.push_back(Step::Issue {
      key: key.to_owned(),
    });
  }
}

/// The step committed: send the babysit prompt it held back, taking the
/// issue out of the queue first.
fn release_held(world: &mut World, key: &str) {
  let Some(held) = world
    .issues
    .get_mut(key)
    .and_then(|issue| issue.queue.held.take())
  else {
    return;
  };
  world.outbox.push_back(Step::Issue {
    key: key.to_owned(),
  });
  if world.issues.get(key).is_some_and(crate::queue::queued) {
    leave(world, key, "babysit", "babysit needs the worker");
  }
  if let Some(note) = &held.note {
    push_attention(world, "babysit", key, note);
  }
  let fallback = held.note.is_none().then_some("babysit");
  crate::babysit::prompt(world, key, "babysit", held.prompt, fallback);
}

/// Check the head's pull request again now; its fixed slot stays.
fn check_now(world: &mut World, key: &str) {
  for watch in world.watches.values_mut() {
    if matches!(&watch.kind, crate::watch::Kind::Pr { key: watched, .. } if watched == key) {
      watch.request_immediate();
    }
  }
}

/// Take `key` out of its queue at `phase` (an armed auto-merge becomes due
/// a disarm, see `disarm`), and arm the repository's next head.
pub(crate) fn leave(world: &mut World, key: &str, phase: &str, note: &str) {
  let now = world.now_ms;
  let Some(issue) = world.issues.get_mut(key) else {
    return;
  };
  phase.clone_into(&mut issue.phase);
  note_phase(issue, phase, now);
  issue.phase_at_ms = now;
  issue.stall_nudged = false;
  issue.deferred = None;
  world.outbox.push_back(Step::Status {
    key: key.to_owned(),
  });
  world.outbox.push_back(Step::Issue {
    key: key.to_owned(),
  });
  journal(
    world,
    "transition",
    "queue-left",
    key,
    &format!("{phase}: {note}"),
  );
  arm_ready(world);
}

fn journal(world: &mut World, kind: &str, name: &str, key: &str, note: &str) {
  world
    .outbox
    .push_back(Step::Journal(Record::new(kind, name, key, "", note)));
}

#[cfg(test)]
#[path = "tests/commit_queue_test.rs"]
mod tests;
