//! The merge-queue fields of an issue (#448): when it joined its repository's
//! queue, and how many rebase rounds the engine and its worker performed.
//! It also tracks the auto-merge the engine armed, so an issue that leaves
//! the queue is disarmed, and a babysit prompt held during a queue step.
//! `queued_at`, `rebase_rounds`, `auto_armed`, `disarm_due` and
//! `held_prompt` live in `issues/<key>.json`, so they survive a restart.

use std::time::{Duration, UNIX_EPOCH};

use serde_json::{Map, Value, json};
use toolu_state::time::{epoch_millis, iso_millis, parse_iso};

use crate::model::Issue;

/// An issue's merge-queue state.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct QueueState {
  /// When the issue joined its repository's merge queue, milliseconds.
  pub queued_at_ms: Option<u64>,
  /// The engine armed GitHub auto-merge and has not disarmed it.
  pub auto_armed: bool,
  /// The issue left the queue with auto-merge armed: disarm it.
  pub disarm_due: bool,
  /// A failed disarm is not tried again before this time.
  pub disarm_at_ms: u64,
  /// Disarms that failed in a row.
  pub disarm_errors: u32,
  /// A babysit prompt that waits for the in-flight queue step to commit.
  pub held: Option<Held>,
  /// Rebase rounds by the engine and by the worker.
  pub rebase_rounds: Rounds,
}

/// A babysit prompt held while the queue step runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Held {
  /// The worker's prompt.
  pub prompt: String,
  /// An escalation's attention note.
  pub note: Option<String>,
}

/// Rebase rounds of one issue, by who performed them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Rounds {
  /// Mechanical rebases the engine pushed.
  pub engine: u32,
  /// `REBASE` hand-offs to the worker.
  pub worker: u32,
}

/// Who performed a rebase round.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum By {
  /// The engine's mechanical rebase.
  Engine,
  /// The worker, after a `REBASE` prompt.
  Worker,
}

impl By {
  /// Journal name.
  pub(crate) fn name(self) -> &'static str {
    match self {
      Self::Engine => "engine",
      Self::Worker => "worker",
    }
  }
}

/// Count one round by `by`; the issue's round number for the journal.
pub(crate) fn count_round(issue: &mut Issue, by: By) -> u32 {
  let rounds = &mut issue.queue.rebase_rounds;
  let slot = match by {
    By::Engine => &mut rounds.engine,
    By::Worker => &mut rounds.worker,
  };
  *slot = slot.saturating_add(1);
  rounds.engine.saturating_add(rounds.worker)
}

/// The phase is now `phase`: a `ready` issue joins the queue at `now_ms`
/// (keeping an earlier time) and its head step owns any auto-merge again;
/// any other phase leaves the queue, and an armed auto-merge is due a disarm.
pub(crate) fn note_phase(issue: &mut Issue, phase: &str, now_ms: u64) {
  let queue = &mut issue.queue;
  if phase == "ready" {
    queue.queued_at_ms.get_or_insert(now_ms);
    queue.disarm_due = false;
  } else {
    queue.queued_at_ms = None;
    if queue.auto_armed {
      queue.disarm_due = true;
    }
  }
}

/// Write the queue fields into the record map.
pub(crate) fn write(map: &mut Map<String, Value>, issue: &Issue) {
  match issue.queue.queued_at_ms {
    Some(ms) => {
      let at = UNIX_EPOCH + Duration::from_millis(ms);
      map.insert("queued_at".to_owned(), json!(iso_millis(at)));
    }
    None => {
      map.remove("queued_at");
    }
  }
  for (name, on) in [
    ("auto_armed", issue.queue.auto_armed),
    ("disarm_due", issue.queue.disarm_due),
  ] {
    if on {
      map.insert(name.to_owned(), json!(true));
    } else {
      map.remove(name);
    }
  }
  match &issue.queue.held {
    Some(held) => {
      map.insert(
        "held_prompt".to_owned(),
        json!({"prompt": held.prompt, "note": held.note}),
      );
    }
    None => {
      map.remove("held_prompt");
    }
  }
  let rounds = issue.queue.rebase_rounds;
  if rounds == Rounds::default() {
    map.remove("rebase_rounds");
  } else {
    map.insert(
      "rebase_rounds".to_owned(),
      json!({"engine": rounds.engine, "worker": rounds.worker}),
    );
  }
}

/// Read the queue fields from a record.
pub(crate) fn read(issue: &mut Issue, record: &Value) {
  issue.queue.queued_at_ms = record
    .get("queued_at")
    .and_then(Value::as_str)
    .and_then(parse_iso)
    .map(epoch_millis);
  let count = |by: &str| {
    record
      .pointer(&format!("/rebase_rounds/{by}"))
      .and_then(Value::as_u64)
      .and_then(|n| u32::try_from(n).ok())
      .unwrap_or(0)
  };
  issue.queue.rebase_rounds = Rounds {
    engine: count("engine"),
    worker: count("worker"),
  };
  issue.queue.auto_armed = record.get("auto_armed") == Some(&Value::Bool(true));
  issue.queue.disarm_due = record.get("disarm_due") == Some(&Value::Bool(true));
  issue.queue.held = record.get("held_prompt").and_then(|held| {
    Some(Held {
      prompt: held.get("prompt")?.as_str()?.to_owned(),
      note: held.get("note").and_then(Value::as_str).map(str::to_owned),
    })
  });
}

#[cfg(test)]
#[path = "tests/queue_fields_test.rs"]
mod tests;
