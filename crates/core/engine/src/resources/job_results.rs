//! Detached job results under `<resource_root>/jobs/results/<id>.json`:
//! one file per job, written atomically, claimed exactly once when done.

use std::io::{BufRead as _, BufReader, Write as _};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde_json::{Value, json};
use toolu_runtime::atomic::replace_atomic;
use toolu_state::lock::token;
use toolu_state::time::iso_millis;

use super::lock::with_resource_lock;

/// One detached job's result record (schema 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
  /// The job id (its file name without `.json`).
  pub id: String,
  /// The owning worker's key; empty when no epic key applies.
  pub key: String,
  /// The bound worktree, absolute.
  pub worktree: String,
  /// The argv, joined with spaces.
  pub label: String,
  /// `waiting`, `running` or `done`.
  pub state: String,
  /// The exit code once `done`; `None` before.
  pub exit_code: Option<i64>,
  /// When the record was created.
  pub started_at: String,
  /// When the job ended.
  pub finished_at: Option<String>,
  /// The text the blocking call prints; empty until `done`.
  pub output: String,
  /// The tree changed under the run (#876); always `false` here.
  pub stale: bool,
  /// When a consumer claimed the finished record.
  pub delivered_at: Option<String>,
}

fn dir(root: &Path) -> PathBuf {
  root.join("jobs/results")
}

fn io(path: &Path, err: impl std::fmt::Display) -> String {
  format!("{}: {err}", path.display())
}

fn path(root: &Path, id: &str) -> Result<PathBuf, String> {
  let safe = !id.is_empty()
    && id
      .chars()
      .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'));
  if !safe {
    return Err(format!("invalid job result id {id}"));
  }
  Ok(dir(root).join(format!("{id}.json")))
}

fn text(value: &Value, key: &str) -> Option<String> {
  value.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn optional_text(value: &Value, key: &str) -> Result<Option<String>, ()> {
  match value.get(key) {
    Some(Value::Null) | None => Ok(None),
    Some(Value::String(text)) => Ok(Some(text.clone())),
    _ => Err(()),
  }
}

fn parse(value: &Value, path: &Path) -> Result<Record, String> {
  let invalid = || format!("invalid job result {}", path.display());
  let schema = value.get("schema").and_then(Value::as_i64);
  let record = Record {
    id: text(value, "id").ok_or_else(invalid)?,
    key: text(value, "key").ok_or_else(invalid)?,
    worktree: text(value, "worktree").ok_or_else(invalid)?,
    label: text(value, "label").ok_or_else(invalid)?,
    state: text(value, "state").ok_or_else(invalid)?,
    exit_code: match value.get("exit_code") {
      Some(Value::Null) | None => None,
      Some(code) => Some(code.as_i64().ok_or_else(invalid)?),
    },
    started_at: text(value, "started_at").ok_or_else(invalid)?,
    finished_at: optional_text(value, "finished_at").map_err(|()| invalid())?,
    output: text(value, "output").ok_or_else(invalid)?,
    stale: value
      .get("stale")
      .and_then(Value::as_bool)
      .ok_or_else(invalid)?,
    delivered_at: optional_text(value, "delivered_at").map_err(|()| invalid())?,
  };
  if schema != Some(1) || !matches!(record.state.as_str(), "waiting" | "running" | "done") {
    return Err(invalid());
  }
  Ok(record)
}

fn to_value(record: &Record) -> Value {
  json!({
    "schema": 1, "id": record.id, "key": record.key, "worktree": record.worktree,
    "label": record.label, "state": record.state, "exit_code": record.exit_code,
    "started_at": record.started_at, "finished_at": record.finished_at,
    "output": record.output, "stale": record.stale, "delivered_at": record.delivered_at,
  })
}

/// Create a `waiting` record and return it with its new id.
///
/// # Errors
/// The record cannot be written.
pub fn create(root: &Path, key: &str, worktree: &Path, label: &str) -> Result<Record, String> {
  let record = Record {
    id: token(),
    key: key.to_owned(),
    worktree: worktree.display().to_string(),
    label: label.to_owned(),
    state: "waiting".to_owned(),
    exit_code: None,
    started_at: iso_millis(SystemTime::now()),
    finished_at: None,
    output: String::new(),
    stale: false,
    delivered_at: None,
  };
  update(root, &record)?;
  Ok(record)
}

/// Write `record` over its file, atomically.
///
/// # Errors
/// The record cannot be serialized or written.
pub fn update(root: &Path, record: &Record) -> Result<(), String> {
  let path = path(root, &record.id)?;
  let body = serde_json::to_string(&to_value(record)).map_err(|err| io(&path, err))?;
  replace_atomic(&path, &body).map_err(|err| io(&path, err))
}

/// Read one record; `None` when it does not exist.
///
/// # Errors
/// The file cannot be read or is not a schema-1 record, or `id` is unsafe.
pub fn get(root: &Path, id: &str) -> Result<Option<Record>, String> {
  let path = path(root, id)?;
  let bytes = match std::fs::read(&path) {
    Ok(bytes) => bytes,
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
    Err(err) => return Err(io(&path, err)),
  };
  let value: Value = serde_json::from_slice(&bytes).map_err(|err| io(&path, err))?;
  parse(&value, &path).map(Some)
}

fn all(root: &Path) -> Result<Vec<Record>, String> {
  let folder = dir(root);
  let entries = match std::fs::read_dir(&folder) {
    Ok(entries) => entries,
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
    Err(err) => return Err(io(&folder, err)),
  };
  let mut records = Vec::new();
  for entry in entries {
    let entry = entry.map_err(|err| io(&folder, err))?;
    let file = entry.path();
    if file.extension().is_some_and(|ext| ext == "json") {
      let bytes = std::fs::read(&file).map_err(|err| io(&file, err))?;
      let value: Value = serde_json::from_slice(&bytes).map_err(|err| io(&file, err))?;
      records.push(parse(&value, &file)?);
    }
  }
  records.sort_by(|a, b| {
    a.started_at
      .cmp(&b.started_at)
      .then_with(|| a.id.cmp(&b.id))
  });
  Ok(records)
}

/// Every undelivered record of `worktree`, oldest first.
///
/// # Errors
/// A record cannot be read or parsed.
pub fn pending(root: &Path, worktree: &Path) -> Result<Vec<Record>, String> {
  let worktree = worktree.display().to_string();
  Ok(
    all(root)?
      .into_iter()
      .filter(|record| record.worktree == worktree && record.delivered_at.is_none())
      .collect(),
  )
}

/// Claim `worktree`'s finished records: each is marked delivered under the
/// resource lock, so two claims return a record once at most.
///
/// # Errors
/// The lock, a record read, or a claim write fails.
pub fn claim_finished(root: &Path, worktree: &Path) -> Result<Vec<Record>, String> {
  with_resource_lock(root, || {
    let mut claimed = Vec::new();
    for mut record in pending(root, worktree)? {
      if record.state == "done" {
        record.delivered_at = Some(iso_millis(SystemTime::now()));
        update(root, &record)?;
        claimed.push(record);
      }
    }
    Ok(claimed)
  })
}

/// Claim one finished result of `worktree`, leaving every other result pending.
///
/// # Errors
/// The result cannot be read or written, or it belongs to another worktree.
pub fn claim_one(root: &Path, worktree: &Path, id: &str) -> Result<Option<Record>, String> {
  with_resource_lock(root, || {
    let Some(mut record) = get(root, id)? else {
      return Ok(None);
    };
    if record.worktree != worktree.display().to_string() {
      return Err(format!("job result {id} belongs to another worktree"));
    }
    if record.state != "done" || record.delivered_at.is_some() {
      return Ok(None);
    }
    if record.exit_code.is_none() {
      return Err(format!("job result {id} has no exit code"));
    }
    record.delivered_at = Some(iso_millis(SystemTime::now()));
    update(root, &record)?;
    Ok(Some(record))
  })
}

/// The line an agent is told about a finished record.
#[must_use]
pub fn line(record: &Record) -> String {
  let code = record
    .exit_code
    .map_or_else(|| "?".to_owned(), |code| code.to_string());
  let stale = if record.stale { ", stale" } else { "" };
  format!(
    "job {} finished: exit {code}{stale}: {}",
    record.id, record.label
  )
}

/// Tell the engine `job-done` for `key`, best effort: connect to its socket,
/// read the greeting, and send the event with the greeting's protocol.
pub fn announce(root: &Path, key: &str) {
  let Ok(mut stream) = UnixStream::connect(root.join("engine.sock")) else {
    return;
  };
  let timeout = Some(Duration::from_secs(2));
  if stream.set_read_timeout(timeout).is_err() || stream.set_write_timeout(timeout).is_err() {
    return;
  }
  let mut reader = BufReader::new(match stream.try_clone() {
    Ok(clone) => clone,
    Err(_) => return,
  });
  let mut greeting = String::new();
  if reader.read_line(&mut greeting).is_err() {
    return;
  }
  let Ok(hello) = serde_json::from_str::<Value>(&greeting) else {
    return;
  };
  let Some(protocol) = hello.get("protocol").and_then(Value::as_u64) else {
    return;
  };
  let event = json!({"op": "event", "key": key, "event": "job-done", "protocol": protocol});
  let _sent = stream.write_all(format!("{event}\n").as_bytes());
}

#[cfg(test)]
#[path = "tests/job_results_test.rs"]
mod tests;
