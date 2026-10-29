//! The strict persistence boundary of `issues/<key>.json` (`launch-state.ts`):
//! a missing record is a new attempt; anything else must be an object with a
//! known stage and well-formed identity fields. Extension fields are kept.

use std::path::Path;

use serde_json::{Map, Value};

/// One launch record.
pub(crate) type Record = Map<String, Value>;

const STAGES: [&str; 9] = [
  "uncertain",
  "replacing",
  "starting",
  "cleaning",
  "cleanup-incomplete",
  "running",
  "awaiting_merge",
  "merged",
  "abandoned",
];

const IDENTITY: [&str; 5] = [
  "lease_token",
  "resource_root",
  "pane_id",
  "worktree",
  "session_id",
];

const MAX_SAFE: f64 = 9_007_199_254_740_991.0;

/// A string field of `record`.
pub(crate) fn text<'a>(record: &'a Record, key: &str) -> Option<&'a str> {
  record.get(key).and_then(Value::as_str)
}

fn ending(record: &Record) -> bool {
  matches!(
    text(record, "stage"),
    Some("cleaning" | "cleanup-incomplete" | "merged" | "abandoned")
  )
}

/// A launch whose prompt was never confirmed may still lack its session: while
/// it starts, and once the engine adopted the worker that reported before its
/// launch confirmed the prompt (#625), through the gate parking it to merge.
fn awaiting_capture(record: &Record) -> bool {
  let prompt = text(record, "prompt_state");
  match text(record, "stage") {
    Some("starting" | "uncertain" | "replacing") => prompt != Some("acknowledged"),
    Some("running" | "awaiting_merge") => prompt.is_some_and(|state| state != "acknowledged"),
    _ => false,
  }
}

fn check_identity(record: &Record, invalid: &dyn Fn(&str) -> String) -> Result<(), String> {
  for field in IDENTITY {
    let Some(value) = record.get(field) else {
      continue;
    };
    if field == "session_id" && value.is_null() {
      if !ending(record) && !awaiting_capture(record) {
        return Err(invalid(
          "session_id may be null only while awaiting capture",
        ));
      }
      continue;
    }
    if value.as_str().is_none_or(|text| text.trim().is_empty()) {
      return Err(invalid(&format!("{field} must be a non-empty string")));
    }
  }
  let managed = text(record, "lease_token").is_some()
    && !ending(record)
    && !awaiting_capture(record)
    && (text(record, "stage") == Some("running")
      || text(record, "prompt_state") == Some("acknowledged"));
  if managed && text(record, "backend") == Some("herdr-app") {
    check_hc(record, invalid)?;
  } else if managed && text(record, "session_id").is_none() {
    return Err(invalid("a managed running launch requires session_id"));
  }
  Ok(())
}

fn check_hc(record: &Record, invalid: &dyn Fn(&str) -> String) -> Result<(), String> {
  let name = text(record, "terminal_name").unwrap_or("");
  let owner = text(record, "terminal_owner").unwrap_or("");
  let address = text(record, "terminal_address").unwrap_or("");
  let session = text(record, "hc_session").unwrap_or("");
  let valid_name = !name.is_empty()
    && name.len() <= 32
    && name
      .bytes()
      .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'-' | b'_'));
  let valid_address =
    !address.is_empty() && !address.starts_with('-') && !address.chars().any(char::is_control);
  if !valid_name
    || !valid_address
    || session.is_empty()
    || session.chars().any(char::is_control)
    || owner.strip_prefix("toolu.owner=") != text(record, "lease_token")
  {
    return Err(invalid(
      "a managed hc launch requires matching terminal identity and session",
    ));
  }
  let target = format!("hc:{address}");
  if text(record, "agent") != Some(target.as_str())
    || text(record, "pane_id") != Some(target.as_str())
  {
    return Err(invalid("hc target differs from terminal address"));
  }
  Ok(())
}

fn check_numbers(record: &Record, invalid: &dyn Fn(&str) -> String) -> Result<(), String> {
  if let Some(launches) = record.get("launches") {
    let whole = launches
      .as_f64()
      .is_some_and(|n| (0.0..=MAX_SAFE).contains(&n) && n.fract() == 0.0);
    if !whole {
      return Err(invalid("launches must be a nonnegative integer"));
    }
  }
  if let Some(since) = record.get("session_started_after")
    && !since.as_f64().is_some_and(|n| n.is_finite() && n >= 0.0)
  {
    return Err(invalid("session_started_after must be a finite epoch"));
  }
  Ok(())
}

/// The validated record at `path`, or an empty one when it does not exist.
///
/// # Errors
/// The file cannot be read, or the record is malformed.
pub(crate) fn load_launch_record(path: &Path) -> Result<Record, String> {
  let source = match std::fs::read_to_string(path) {
    Ok(source) => source,
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Record::new()),
    Err(err) => {
      return Err(format!(
        "cannot read launch record {}: {err}",
        path.display()
      ));
    }
  };
  let invalid = |detail: &str| format!("invalid launch record {}: {detail}", path.display());
  let value: Value = serde_json::from_str(&source).map_err(|_err| invalid("malformed JSON"))?;
  let Value::Object(record) = value else {
    return Err(invalid("expected a JSON object"));
  };
  if let Some(stage) = record.get("stage")
    && !stage.as_str().is_some_and(|stage| STAGES.contains(&stage))
  {
    return Err(invalid(&format!("unknown stage {stage}")));
  }
  check_numbers(&record, &invalid)?;
  check_identity(&record, &invalid)?;
  Ok(record)
}

/// Write `record` to `path` as pretty JSON, atomically.
///
/// # Errors
/// The directory or file cannot be written.
pub(crate) fn write_record(path: &Path, record: &Record) -> Result<(), String> {
  crate::disk::write_pretty(path, record)
}

#[cfg(test)]
#[path = "tests/record_test.rs"]
mod tests;
