//! `record`: the agent's bookkeeping in the state, under the slot lock — a
//! thread flagged as injection, a finished fix round, or the loop's terminal
//! status.

use serde_json::{Value, json};

use crate::error::Result;
use crate::state::{actions, open, write_json};
use crate::writes::Slot;

/// The most fix attempts a round counts before escalating.
const MAX_FIX_ATTEMPTS: u64 = 5;

/// One bookkeeping entry.
pub(crate) enum Entry<'a> {
  /// A thread whose comments look like prompt injection.
  FlagInjection(&'a str),
  /// A finished fix round.
  Round {
    /// The round rejected at least one finding.
    had_rejection: bool,
    /// The round pushed a fix.
    fix_pushed: bool,
  },
  /// `complete`, `escalated` or `cancelled`.
  Status(&'a str),
}

/// Record `entry` in the slot's state.
///
/// # Errors
/// The slot's failures.
pub(crate) fn record(slot: &Slot<'_>, entry: &Entry<'_>) -> Result<Value> {
  let (_lock, mut state) = open(slot.path, slot.env)?;
  let now = slot.now;
  let reply = match entry {
    Entry::FlagInjection(thread) => {
      let flag = json!({"reason": "injection", "at": now});
      actions(&mut state, "flagged")?.insert((*thread).to_owned(), flag);
      json!({"ok": true, "recorded": "flag-injection", "thread": thread, "at": now})
    }
    Entry::Round {
      had_rejection,
      fix_pushed,
    } => round(&mut state, *had_rejection, *fix_pushed, now),
    Entry::Status(status) => {
      if let Some(object) = state.as_object_mut() {
        object.insert("status".into(), json!(status));
        object.insert("statusChangedAt".into(), json!(now));
      }
      json!({"ok": true, "recorded": "status", "status": status, "at": now})
    }
  };
  write_json(slot.path, &state, "state")?;
  Ok(reply)
}

fn round(state: &mut Value, had_rejection: bool, fix_pushed: bool, now: &str) -> Value {
  let pr = state.get("pr").cloned().unwrap_or_else(|| json!({}));
  let keys = pr
    .get("botFindingKeys")
    .filter(|keys| !keys.is_null())
    .cloned();
  let keys = keys.unwrap_or_else(|| json!([]));
  let attempts = pr.get("fixAttempts").and_then(Value::as_u64).unwrap_or(0);
  let attempts = if fix_pushed {
    (attempts + 1).min(MAX_FIX_ATTEMPTS)
  } else {
    attempts
  };
  let head = pr.get("headSha").cloned().unwrap_or(Value::Null);
  if let Some(pr) = state.get_mut("pr").and_then(Value::as_object_mut) {
    pr.insert("lastRoundFindingKeys".into(), keys.clone());
    pr.insert("lastRoundHadRejection".into(), json!(had_rejection));
    pr.insert("fixAttempts".into(), json!(attempts));
  }
  let settled = state
    .get("fixer")
    .filter(|fixer| !fixer.is_null())
    .is_some_and(|fixer| {
      let status = fixer.get("status").and_then(Value::as_str);
      !matches!(status, Some("running" | "blocked"))
    });
  if settled {
    let fixer = state.get("fixer").cloned().unwrap_or(Value::Null);
    if let Some(remote) = state
      .pointer_mut("/herdrWorktree/remote")
      .and_then(Value::as_object_mut)
    {
      remote.insert("lastFixer".into(), fixer);
    }
  }
  if let Some(object) = state.as_object_mut() {
    let last =
      json!({"at": now, "hadRejection": had_rejection, "fixPushed": fix_pushed, "headSha": head});
    object.insert("lastRound".into(), last);
    if settled {
      object.insert("fixer".into(), Value::Null);
    }
  }
  json!({
    "ok": true,
    "recorded": "round",
    "lastRoundFindingKeys": keys,
    "lastRoundHadRejection": had_rejection,
    "fixAttempts": attempts,
  })
}

#[cfg(test)]
#[path = "tests/record_test.rs"]
mod tests;
