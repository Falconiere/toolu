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
  if managed && text(record, "session_id").is_none() {
    return Err(invalid("a managed running launch requires session_id"));
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
  require_local(&value, path)?;
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

/// Preserve recorded local backend choices across configuration changes.
pub(crate) fn backend(
  record: &Record,
  env: &toolu_runtime::env::Env,
) -> Result<toolu_mux::Backend, super::Failure> {
  match text(record, "backend") {
    Some("herdr") => Ok(toolu_mux::Backend::Herdr),
    Some("tmux") => Ok(toolu_mux::Backend::Tmux),
    Some(_) => Err(super::Failure::refusal("unknown recorded worker backend")),
    None if !record.is_empty() => Ok(
      if text(record, "pane_id").is_some_and(|pane| pane.starts_with('%')) {
        toolu_mux::Backend::Tmux
      } else {
        toolu_mux::Backend::Herdr
      },
    ),
    None => toolu_mux::backend::configured(env).map_err(super::Failure::refusal),
  }
}

/// Pin a launch to its persisted local manager before any reservation.
pub(crate) fn pin(
  record: &mut Record,
  client: &mut crate::herdr::Client,
  env: &toolu_runtime::env::Env,
) -> Result<(), super::Failure> {
  let backend = backend(record, env)?;
  client.pin(backend);
  if record.is_empty() {
    record.insert("backend".into(), Value::String(backend.name().into()));
  }
  Ok(())
}

/// Write `record` to `path` as pretty JSON, atomically.
///
/// # Errors
/// The directory or file cannot be written.
pub(crate) fn write_record(path: &Path, record: &Record) -> Result<(), String> {
  require_local_record(record, path)?;
  crate::disk::write_pretty(path, record)
}

/// A retired address cannot be sent to a local pane manager.
pub(crate) fn retired_target(target: &str) -> bool {
  target.starts_with("hc:") || target.starts_with("relay:")
}

fn unsupported(path: &Path) -> String {
  format!(
    "unsupported retired worker state at {}; settle and remove this registration with the pre-removal toolu binary before restarting or using this state",
    path.display()
  )
}

/// Keep unsupported records byte-for-byte intact for the old binary.
pub(crate) fn require_local(value: &Value, path: &Path) -> Result<(), String> {
  match value.as_object() {
    Some(record) => require_local_record(record, path),
    None => Ok(()),
  }
}

/// Check a record without changing its extension fields.
pub(crate) fn require_local_record(record: &Map<String, Value>, path: &Path) -> Result<(), String> {
  let retired = record.get("backend").and_then(Value::as_str) == Some("herdr-app")
    || ["agent", "pane_id", "workspace_id"].iter().any(|key| {
      record
        .get(*key)
        .and_then(Value::as_str)
        .is_some_and(retired_target)
    })
    || ["relay", "relay_enabled"]
      .iter()
      .any(|key| record.get(*key) == Some(&Value::Bool(true)))
    || record.keys().any(|key| {
      key.starts_with("terminal_")
        || key.starts_with("hc_")
        || key.starts_with("remote_")
        || matches!(
          key.as_str(),
          "relay_request"
            | "machine"
            | "expected_base"
            | "controller_review"
            | "review_prompt_state"
        )
    });
  if retired {
    Err(unsupported(path))
  } else {
    Ok(())
  }
}

/// Refuse an old target or active graph record before registering or loading it.
pub(crate) fn require_local_state(dir: &Path) -> Result<(), String> {
  let target = dir.join("target.json");
  if target
    .try_exists()
    .map_err(|err| format!("{}: {err}", target.display()))?
  {
    return Err(unsupported(&target));
  }
  let settings = dir.join("launch.json");
  require_local(&crate::disk::read_value(&settings)?, &settings)?;
  let graph = crate::disk::read_value(&dir.join("graph.json"))?;
  if let Some(issues) = graph.get("issues").and_then(Value::as_array) {
    for key in issues
      .iter()
      .filter_map(|issue| issue.get("key").and_then(Value::as_str))
    {
      let path = dir.join("issues").join(format!("{key}.json"));
      require_local(&crate::disk::read_value(&path)?, &path)?;
    }
  }
  Ok(())
}

/// The shared engine refuses the whole registry before contacting an old engine.
pub(crate) fn require_local_registry(paths: &crate::paths::Paths) -> Result<(), String> {
  let registry = crate::disk::read_value(&paths.registry())?;
  if let Some(epics) = registry.get("epics").and_then(Value::as_array) {
    for dir in epics
      .iter()
      .filter_map(|epic| epic.get("state_dir").and_then(Value::as_str))
    {
      require_local_state(Path::new(dir))?;
    }
  }
  Ok(())
}

#[cfg(test)]
#[path = "tests/record_test.rs"]
mod tests;
