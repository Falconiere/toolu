//! Commit the merge-queue head step (#448): its outcome document becomes
//! engine state, journal lines and, only on a conflict, a `REBASE` prompt.
//! A merged head goes the way of every merge (stage `cleaning`, then
//! cleanup); an engine rebase, a wait or a refusal to touch the worker keep
//! it at the head; a conflict or a failing check takes it out of the queue,
//! and the next head is armed.

use serde_json::{Value, json};

use crate::journal::Record;
use crate::logic::{arm_ready, push_attention, token};
use crate::model::{Action, Step, World, fresh};
use crate::queue_fields::{By, count_round, note_phase};

/// Commit `doc`, the head step's outcome, or `None` when it is not one.
pub(crate) fn commit(world: &mut World, key: &str, doc: &str) -> Option<Vec<Step>> {
  let doc: Value = serde_json::from_str(doc).ok()?;
  let outcome = doc.get("outcome").and_then(Value::as_str)?.to_owned();
  let text = |name: &str| doc.get(name).and_then(Value::as_str).unwrap_or("").to_owned();
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
    "disarmed" => set_armed(world, key, false),
    "busy" => journal(world, "action", "rebase-skipped", key, &note),
    "handoff" => {
      set_armed(world, key, false);
      round(world, key, By::Worker, "");
      leave(world, key, "rebasing", &note);
      crate::babysit::prompt(world, key, "rebase", "REBASE".to_owned(), Some("rebase"));
    }
    "fix" => {
      set_armed(world, key, false);
      let reasons = doc.get("reasons").map(Value::to_string).unwrap_or_default();
      leave(world, key, "babysit", &reasons);
    }
    "closed" => {
      if let Some(issue) = world.issues.get_mut(key) {
        "closed".clone_into(&mut issue.stage);
      }
      world.outbox.push_back(Step::Issue { key: key.to_owned() });
      push_attention(world, "pr-closed", key, "pull request closed");
      arm_ready(world);
    }
    "uncertain" | "refused" => {
      if let Some(issue) = world.issues.get_mut(key) {
        issue.deferred = Some("merge".to_owned());
      }
      push_attention(world, "failed", key, &format!("merge {outcome}: {note}"));
    }
    _ => {}
  }
  Some(world.outbox.drain(..).collect())
}

/// End the pending step without a stage change; its done line carries the
/// outcome, so a restart neither runs it again nor finishes it as a merge.
fn close(world: &mut World, key: &str, outcome: &str, note: &str) {
  let Some(pending) = world.issues.get_mut(key).and_then(|issue| issue.pending.take()) else {
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
  let mut note = json!({"by": by.name(), "round": round});
  if !head.is_empty() {
    note["head"] = json!(head);
  }
  journal(world, "action", "rebase", key, &note.to_string());
  world.outbox.push_back(Step::Issue { key: key.to_owned() });
}

fn set_armed(world: &mut World, key: &str, armed: bool) {
  if let Some(issue) = world.issues.get_mut(key) {
    issue.auto_armed = armed;
  }
}

/// Check the head's pull request again now; its fixed slot stays.
fn check_now(world: &mut World, key: &str) {
  for watch in world.watches.values_mut() {
    if matches!(&watch.kind, crate::watch::Kind::Pr { key: watched, .. } if watched == key) {
      watch.request_immediate();
    }
  }
}

/// Take `key` out of its queue at `phase`, disarm any auto-merge the engine
/// armed, and arm the repository's next head.
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
  let disarm = issue.auto_armed && issue.pending.is_none();
  if disarm {
    let token = token(world);
    if let Some(issue) = world.issues.get_mut(key) {
      issue.pending = Some(fresh(Action::Merge, token));
    }
  }
  world.outbox.push_back(Step::Status { key: key.to_owned() });
  world.outbox.push_back(Step::Issue { key: key.to_owned() });
  journal(world, "transition", "queue-left", key, &format!("{phase}: {note}"));
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
