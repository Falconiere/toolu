//! JSON files under the resource root and an epic state directory. Existing
//! object keys are kept, so a TypeScript reader still sees what it wrote.

use std::path::Path;

use serde_json::{Map, Value, json};
use toolu_runtime::atomic::write_atomic;
use toolu_state::time::iso_seconds;

use crate::model::Issue;
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
  // A corrupt or non-object previous status starts over, as `report.ts` did.
  let mut map = object(read_value(path).unwrap_or(Value::Null));
  let note = bounded_note(note);
  let stamp = iso_seconds(now);
  let duplicate = recorded(&map, phase, pr, &note);
  map.insert("phase".to_owned(), json!(phase));
  // `pr` is always present, `null` until a report names one, as `report.ts`
  // wrote it; a report without `--pr` keeps the recorded one.
  match pr {
    Some(pr) => {
      map.insert("pr".to_owned(), json!(pr));
    }
    None => {
      map.entry("pr").or_insert(Value::Null);
    }
  }
  map.insert("note".to_owned(), json!(note));
  map.insert("updated_at".to_owned(), json!(stamp));
  if !duplicate {
    let mut history = map
      .get("history")
      .and_then(Value::as_array)
      .cloned()
      .unwrap_or_default();
    history.push(json!({"phase": phase, "at": stamp, "note": note}));
    map.insert("history".to_owned(), Value::Array(history));
  }
  write_value(path, &Value::Object(map))
}

/// A row with this phase and note already exists, and `pr` is unchanged.
fn recorded(map: &Map<String, Value>, phase: &str, pr: Option<u64>, note: &str) -> bool {
  let same_pr = match pr {
    None => true,
    Some(pr) => map.get("pr").and_then(Value::as_u64) == Some(pr),
  };
  same_pr
    && map
      .get("history")
      .and_then(Value::as_array)
      .is_some_and(|rows| {
        rows.iter().any(|row| {
          row.get("phase").and_then(Value::as_str) == Some(phase)
            && row.get("note").and_then(Value::as_str) == Some(note)
        })
      })
}

/// Write `value` to `path` as pretty JSON with a trailing newline, atomically,
/// creating the directory.
///
/// # Errors
/// The directory or file cannot be written.
pub(crate) fn write_pretty<T: serde::Serialize>(path: &Path, value: &T) -> Result<(), String> {
  if let Some(parent) = path.parent() {
    std::fs::create_dir_all(parent).map_err(|err| format!("{}: {err}", parent.display()))?;
  }
  let mut text = serde_json::to_string_pretty(value).map_err(|err| err.to_string())?;
  text.push('\n');
  if write_atomic(path, &text) {
    Ok(())
  } else {
    Err(format!("could not write {}", path.display()))
  }
}

/// Write the engine's fields of an issue record: its stage (when known),
/// its coupling hold, deferred action, whether a hold was released, and a waiting relaunch.
/// A record a finish settled (`merged` or `abandoned`) keeps its stage
/// (#619).
///
/// # Errors
/// The record cannot be read or written.
pub(crate) fn write_issue_fields(path: &Path, issue: &Issue) -> Result<(), String> {
  let mut map = object(read_value(path)?);
  let settled = matches!(
    map.get("stage").and_then(Value::as_str),
    Some("merged" | "abandoned")
  );
  if !issue.stage.is_empty() && !settled {
    map.insert("stage".to_owned(), json!(issue.stage));
  }
  match &issue.hold {
    Some(hold) => map.insert("hold".to_owned(), hold.clone()),
    None => map.remove("hold"),
  };
  if issue.coupling_released {
    map.insert("coupling_released".to_owned(), json!(true));
  } else {
    map.remove("coupling_released");
  }
  match &issue.relaunch {
    Some(relaunch) => map.insert("relaunch".to_owned(), json!(relaunch)),
    None => map.remove("relaunch"),
  };
  match &issue.deferred {
    Some(action) => map.insert("deferred".to_owned(), json!(action)),
    None => map.remove("deferred"),
  };
  crate::queue_fields::write(&mut map, issue);
  write_value(path, &Value::Object(map))
}

/// Write `issue`'s engine fields and tracker sighting to its record, unless
/// it has nothing to record yet. A `running` issue's agent lease follows
/// when its launch never confirmed the prompt (#625), unless the engine's
/// own launch of it is pending, which sets the stage when it commits.
///
/// # Errors
/// The record cannot be read or written.
pub(crate) fn write_issue(issue: &Issue, now: std::time::SystemTime) -> Result<(), String> {
  if issue.stage.is_empty()
    && issue.hold.is_none()
    && issue.deferred.is_none()
    && !issue.coupling_released
    && issue.queue.queued_at_ms.is_none()
  {
    return Ok(());
  }
  let path = crate::snapshot::issue_path(&issue.state_dir, &issue.key);
  write_issue_fields(&path, issue)?;
  let launching = issue
    .pending
    .as_ref()
    .is_some_and(|pending| pending.action == crate::model::Action::Launch);
  if issue.stage == "running" && !launching {
    // Best effort, like the dead-lease reclaim: a lease left as it was is
    // mirrored again by the next write, and the record write stands.
    let _mirrored =
      crate::launch::adoption::mirror_running(Path::new(&issue.state_dir), &issue.key);
  }
  write_tracker_seen(&path, issue.tracker_seen, now)
}

/// Record what the engine saw of a merged issue's tracker item: when it was
/// first seen closed, and that its one `reopened` judgment was raised.
///
/// # Errors
/// The record cannot be read or written.
pub(crate) fn write_tracker_seen(
  path: &Path,
  seen: crate::model::Seen,
  now: std::time::SystemTime,
) -> Result<(), String> {
  let mut map = object(read_value(path)?);
  let changed = match seen {
    crate::model::Seen::Open => false,
    crate::model::Seen::Closed => !map.contains_key("tracker_closed_at"),
    crate::model::Seen::Reopened => map.get("reopened_raised") != Some(&Value::Bool(true)),
  };
  if !changed {
    return Ok(());
  }
  if !map.contains_key("tracker_closed_at") {
    let at = toolu_state::time::iso_millis(now);
    map.insert("tracker_closed_at".to_owned(), json!(at));
  }
  if seen == crate::model::Seen::Reopened {
    map.insert("reopened_raised".to_owned(), json!(true));
  }
  write_value(path, &Value::Object(map))
}

#[cfg(test)]
#[path = "tests/disk_test.rs"]
mod tests;
