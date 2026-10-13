//! Journal what a blocking `toolu epic wait` handed its caller (#451): each
//! reply wakes the orchestrator LLM, so `judgment:<kind>` is a wake-up with a
//! decision in it and `timeout` is one without. The line is appended straight
//! to the journal after the reply was sent, so a journal failure never holds
//! a reply back and nothing re-enters the engine outbox mid-dispatch.

use serde_json::Value;

use crate::journal::{self, Record};
use crate::server::Engine;

/// Journal one reply `body` to a blocking wait.
///
/// # Errors
/// The journal cannot be written.
pub(crate) fn replied(engine: &Engine, body: &Value) -> Result<(), String> {
  let text = |field: &str| body.get(field).and_then(Value::as_str).unwrap_or("");
  let mut record = match body.get("kind").and_then(Value::as_str) {
    Some(kind) => Record::new(
      "action",
      "wait-reply",
      text("key"),
      "",
      &format!("judgment:{kind}"),
    ),
    None => Record::new("action", "wait-reply", "", "", "timeout"),
  };
  if !record.key.is_empty() {
    text("epic").clone_into(&mut record.epic);
  }
  journal::append(
    &engine.paths.journal_dir(),
    &engine.paths.journal_lock(),
    &record,
    engine.now,
  )
  .map(|_| ())
}

#[cfg(test)]
#[path = "tests/wait_journal_test.rs"]
mod tests;
