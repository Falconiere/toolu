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

/// Deliver an answer to the worker for the `answer` op.
pub(crate) fn reply_answer(engine: &mut Engine, request: &Value, reply: &Sender<Value>) -> bool {
  let key = request.get("key").and_then(Value::as_str).unwrap_or("");
  let text = request.get("text").and_then(Value::as_str).unwrap_or("");
  let by = request.get("by").and_then(Value::as_str).unwrap_or("user");
  let result = if key.is_empty() || text.is_empty() {
    Err("answer needs a key and a text".to_owned())
  } else if crate::attention::has_live(&engine.world, key) {
    answer(engine, key, by, text)
  } else {
    Err(format!("no open attention for {key}"))
  };
  let _sent = reply.send(result_value(result));
  false
}

/// Prompt `key`'s worker with `text`, then mark its attention answered.
fn answer(engine: &mut Engine, key: &str, by: &str, text: &str) -> Result<(), String> {
  engine
    .send_prompt(key, text)
    .map_err(|err| format!("worker prompt not delivered: {err}"))?;
  crate::attention::answered(&mut engine.world, key, by, text);
  engine.flush()
}

#[cfg(test)]
#[path = "tests/attention_ops_test.rs"]
mod tests;
