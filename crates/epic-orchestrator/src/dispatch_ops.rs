//! Socket operations protocol 2 adds: `release {key}` clears a coupling
//! hold so the issue launches next.

use std::sync::mpsc::Sender;

use serde_json::{Value, json};

use crate::dispatch::pump_after;
use crate::server::Engine;

/// Release `request.key`'s hold, or answer why not.
///
/// # Errors
/// A journal or record write fails.
pub(crate) fn reply_release(
  engine: &mut Engine,
  request: &Value,
  reply: &Sender<Value>,
) -> Result<bool, String> {
  let key = request.get("key").and_then(Value::as_str).unwrap_or("");
  if let Err(err) = crate::holds::release(&mut engine.world, key) {
    let _sent = reply.send(json!({"error": err}));
    return Ok(false);
  }
  engine.flush()?;
  let _sent = reply.send(json!({"ok": true, "key": key, "released": true}));
  pump_after(engine)
}

#[cfg(test)]
#[path = "tests/dispatch_ops_test.rs"]
mod tests;
