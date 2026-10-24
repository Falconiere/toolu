//! The `ack` and `answer` socket operations on the attention queue.

use std::sync::mpsc::Sender;

use serde_json::{Value, json};

use crate::dispatch::result_value;
use crate::schedule::ack;
use crate::server::Engine;

/// Acknowledge the key's attention items for the `ack` op.
pub(crate) fn reply_ack(
  engine: &mut Engine,
  request: &Value,
  reply: &Sender<Value>,
) -> Result<bool, String> {
  let key = request.get("key").and_then(Value::as_str).unwrap_or("");
  let by = request.get("by").and_then(Value::as_str).unwrap_or("user");
  ack(&mut engine.world, key, by);
  engine.flush()?;
  let _sent = reply.send(json!({"ok": true}));
  Ok(false)
}

/// Deliver an answer to a live worker for the `answer` op, then mark the key's
/// attention answered, or record a direction when none is open. A refused or undelivered answer is the caller's error.
///
/// # Errors
/// The journal cannot be written after the worker received the answer.
pub(crate) fn reply_answer(
  engine: &mut Engine,
  request: &Value,
  reply: &Sender<Value>,
) -> Result<bool, String> {
  let key = request.get("key").and_then(Value::as_str).unwrap_or("");
  let text = request.get("text").and_then(Value::as_str).unwrap_or("");
  let by = request.get("by").and_then(Value::as_str).unwrap_or("user");
  let refused = if key.is_empty() || text.is_empty() {
    Some("answer needs a key and a text".to_owned())
  } else if !crate::attention::has_live_agent(&engine.world, key) {
    Some(format!("no live agent for {key}"))
  } else {
    send_prompt(engine, key, text)
      .err()
      .map(|err| format!("worker prompt not delivered: {err}"))
  };
  if let Some(error) = refused {
    let _sent = reply.send(result_value(Err(error)));
    return Ok(false);
  }
  if crate::attention::has_live(&engine.world, key) {
    crate::attention::answered(&mut engine.world, key, by, text);
  } else {
    crate::attention::directed(&mut engine.world, key, by, text);
  }
  engine.flush()?;
  let _sent = reply.send(result_value(Ok(())));
  Ok(false)
}

/// Send `text` to `key`'s worker through herdr, or the `prompt` script,
/// which reads `{"key","text"}`.
///
/// # Errors
/// herdr fails, or the script fails or is deferred.
pub(crate) fn send_prompt(engine: &Engine, key: &str, text: &str) -> Result<(), String> {
  if engine.scripts.is_none() && crate::source::prompt_enabled() {
    let target = engine
      .world
      .issues
      .get(key)
      .map_or_else(|| key.to_owned(), |issue| issue.agent.clone());
    return crate::source::prompt(&engine.paths, &engine.env, &target, text);
  }
  let body = json!({"key": key, "text": text}).to_string();
  let outcome = crate::effects::invoke(
    engine.scripts.as_deref(),
    crate::model::Action::Prompt,
    &body,
  );
  match outcome.as_str() {
    outcome @ ("failed" | "deferred") => Err(format!("prompt {outcome}")),
    _ => Ok(()),
  }
}

#[cfg(test)]
#[path = "tests/attention_ops_test.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/attention_redirect_test.rs"]
mod redirect_tests;
