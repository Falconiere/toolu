//! Durable comment intent for tracker closure. The remote comment is the
//! authority when a POST's outcome was not recorded locally.

use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};
use toolu_engine::resources::lock::acquire_lock;

const LOCK_WAIT: Duration = Duration::from_secs(10);

/// Confirm one close comment before its separate state transition.
pub(super) fn comment(
  state_dir: &Path,
  target: (&str, &str, &str),
  note: &str,
  present: impl FnOnce(&str) -> Result<bool, String>,
  post: impl FnOnce(&str) -> Result<(), String>,
) -> Result<(), String> {
  let (provider, role, reference) = target;
  let mut encoded = String::with_capacity(reference.len() * 2);
  for byte in reference.bytes() {
    encoded.push(char::from_digit(u32::from(byte >> 4), 16).ok_or("invalid hex digit")?);
    encoded.push(char::from_digit(u32::from(byte & 15), 16).ok_or("invalid hex digit")?);
  }
  let path = state_dir
    .join("close-intents")
    .join(format!("{provider}-{role}-{encoded}.json"));
  let lock_path = path.with_extension("lock");
  let lock = acquire_lock(&lock_path, LOCK_WAIT)?
    .ok_or_else(|| format!("{}: close intent busy; retry later", path.display()))?;
  let result = (|| {
    let (body, delivered) = original_note(&path, note)?;
    if delivered {
      return Ok(());
    }
    if !present(&body)? {
      post(&body)?;
    }
    crate::disk::write_value(&path, &json!({"note": body, "delivered": true}))
  })();
  lock.release();
  result
}

fn original_note(path: &Path, note: &str) -> Result<(String, bool), String> {
  let invalid = || format!("{}: invalid close intent", path.display());
  match crate::disk::read_value(path)? {
    Value::Null if !path.exists() && !note.is_empty() => {
      crate::disk::write_value(path, &json!({"note": note, "delivered": false}))?;
      Ok((note.to_owned(), false))
    }
    Value::Object(fields) => {
      let body = fields.get("note").and_then(Value::as_str);
      let delivered = fields.get("delivered").and_then(Value::as_bool);
      match (body, delivered) {
        (Some(body), Some(delivered)) if !body.is_empty() => Ok((body.to_owned(), delivered)),
        _ => Err(invalid()),
      }
    }
    Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Array(_) => {
      Err(invalid())
    }
  }
}

#[cfg(test)]
#[path = "tests/close_intent_test.rs"]
mod tests;
