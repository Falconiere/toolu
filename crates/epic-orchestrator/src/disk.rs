//! JSON files under the resource root and an epic state directory. Existing
//! object keys are kept, so a TypeScript reader still sees what it wrote.

use std::path::Path;

use serde_json::{Map, Value, json};
use toolu_runtime::atomic::write_atomic;
use toolu_state::time::iso_seconds;

use crate::note::bounded_note;

/// `path` as JSON, or null when it is absent.
///
/// # Errors
/// The file exists and is not JSON, or it cannot be read.
pub(crate) fn read_value(path: &Path) -> Result<Value, String> {
  match std::fs::read_to_string(path) {
    Ok(text) if text.trim().is_empty() => Ok(Value::Null),
    Ok(text) => serde_json::from_str(&text).map_err(|err| format!("{}: {err}", path.display())),
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Value::Null),
    Err(err) => Err(format!("{}: {err}", path.display())),
  }
}

/// Atomically replace `path` with `value` and a trailing newline.
///
/// # Errors
/// The value cannot be encoded or the replace fails.
pub(crate) fn write_value(path: &Path, value: &Value) -> Result<(), String> {
  let mut text = serde_json::to_string(value).map_err(|err| err.to_string())?;
  text.push('\n');
  if write_atomic(path, &text) {
    Ok(())
  } else {
    Err(format!("could not write {}", path.display()))
  }
}

fn object(value: Value) -> Map<String, Value> {
  match value {
    Value::Object(map) => map,
    Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Array(_) => {
      Map::new()
    }
  }
}

/// Update a status file the way `report.ts` does, keeping every other key.
///
/// # Errors
/// The file cannot be read or replaced.
pub(crate) fn write_status(
  path: &Path,
  phase: &str,
  pr: Option<u64>,
  note: &str,
  now: std::time::SystemTime,
) -> Result<(), String> {
  let mut map = object(read_value(path)?);
  let note = bounded_note(note);
  let stamp = iso_seconds(now);
  map.insert("phase".to_owned(), json!(phase));
  if let Some(pr) = pr {
    map.insert("pr".to_owned(), json!(pr));
  }
  map.insert("note".to_owned(), json!(note));
  map.insert("updated_at".to_owned(), json!(stamp));
  let mut history = map
    .get("history")
    .and_then(Value::as_array)
    .cloned()
    .unwrap_or_default();
  history.push(json!({"phase": phase, "at": stamp, "note": note}));
  map.insert("history".to_owned(), Value::Array(history));
  write_value(path, &Value::Object(map))
}

/// Set `stage` on an issue record, keeping every other key.
///
/// # Errors
/// The file cannot be read or replaced.
pub(crate) fn write_stage(path: &Path, stage: &str) -> Result<(), String> {
  let mut map = object(read_value(path)?);
  map.insert("stage".to_owned(), json!(stage));
  write_value(path, &Value::Object(map))
}

#[cfg(test)]
#[path = "tests/disk_test.rs"]
mod tests;
