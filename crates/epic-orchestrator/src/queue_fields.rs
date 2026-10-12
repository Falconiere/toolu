//! The merge-queue fields of an issue (#448): when it joined its repository's
//! queue, and how many rebase rounds the engine and its worker performed.
//! Both live in `issues/<key>.json` (`queued_at`, `rebase_rounds`), so the
//! queue order and the counts survive an engine restart.

use std::time::{Duration, UNIX_EPOCH};

use serde_json::{Map, Value, json};
use toolu_state::time::{epoch_millis, iso_millis, parse_iso};

use crate::model::Issue;

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
  let rounds = &mut issue.rebase_rounds;
  let slot = match by {
    By::Engine => &mut rounds.engine,
    By::Worker => &mut rounds.worker,
  };
  *slot = slot.saturating_add(1);
  rounds.engine.saturating_add(rounds.worker)
}

/// The phase is now `phase`: a `ready` issue joins the queue at `now_ms`
/// (keeping an earlier time), any other phase leaves it.
pub(crate) fn note_phase(issue: &mut Issue, phase: &str, now_ms: u64) {
  if phase == "ready" {
    issue.queued_at_ms.get_or_insert(now_ms);
  } else {
    issue.queued_at_ms = None;
  }
}

/// Write the queue fields into the record map.
pub(crate) fn write(map: &mut Map<String, Value>, issue: &Issue) {
  match issue.queued_at_ms {
    Some(ms) => {
      let at = UNIX_EPOCH + Duration::from_millis(ms);
      map.insert("queued_at".to_owned(), json!(iso_millis(at)));
    }
    None => {
      map.remove("queued_at");
    }
  }
  let rounds = issue.rebase_rounds;
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
  issue.queued_at_ms = record
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
  issue.rebase_rounds = Rounds {
    engine: count("engine"),
    worker: count("worker"),
  };
}

#[cfg(test)]
#[path = "tests/queue_fields_test.rs"]
mod tests;
