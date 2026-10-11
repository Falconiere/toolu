//! Brief waiting-job records shared by Rust and Bun worker wrappers.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde_json::{Value, json};
use toolu_engine::resources::binding::ResourceBinding;
use toolu_state::time::{epoch_millis, parse_iso};

use crate::disk::{read_value, write_value};

const STALE: Duration = Duration::from_secs(120);

fn file(root: &Path, key: &str) -> PathBuf {
  let safe: String = key
    .chars()
    .filter(|letter| letter.is_ascii_alphanumeric() || *letter == '-')
    .take(80)
    .collect();
  root.join("waiting-jobs").join(format!("{safe}.json"))
}

/// Record a refused job admission without changing the resource lease store.
///
/// # Errors
/// The sidecar cannot be written.
pub(crate) fn mark(binding: &ResourceBinding, now: SystemTime) -> Result<(), String> {
  let path = file(&binding.root, &binding.key);
  let earlier = read_value(&path)?;
  let first_wait_at = earlier
    .get("first_wait_at")
    .and_then(Value::as_str)
    .and_then(parse_iso)
    .map_or_else(
      || toolu_state::time::iso_millis(now),
      toolu_state::time::iso_millis,
    );
  write_value(
    &path,
    &json!({
      "schema": 1, "key": binding.key,
      "epic": crate::control::epic_key(Path::new(&binding.state_dir)),
      "worktree": binding.worktree,
      "first_wait_at": first_wait_at,
      "last_attempt_at": toolu_state::time::iso_millis(now),
    }),
  )
}

/// Remove a waiting marker after admission or completion.
pub(crate) fn clear(binding: &ResourceBinding) {
  let _removed = std::fs::remove_file(file(&binding.root, &binding.key));
}

/// Read fresh waiting markers as allow-listed status rows.
///
/// # Errors
/// The sidecar directory or a present marker cannot be read.
pub(crate) fn list(root: &Path, now_ms: u64) -> Result<Vec<Value>, String> {
  let directory = root.join("waiting-jobs");
  let entries = match std::fs::read_dir(&directory) {
    Ok(entries) => entries,
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
    Err(err) => return Err(format!("{}: {err}", directory.display())),
  };
  let mut rows = Vec::new();
  for entry in entries {
    let entry = entry.map_err(|err| err.to_string())?;
    if entry
      .path()
      .extension()
      .is_none_or(|extension| extension != "json")
    {
      continue;
    }
    let record = read_value(&entry.path())?;
    if let Some(row) = public_row(&record, now_ms) {
      rows.push(row);
    }
  }
  rows.sort_by(|left, right| left["key"].as_str().cmp(&right["key"].as_str()));
  Ok(rows)
}

fn public_row(record: &Value, now_ms: u64) -> Option<Value> {
  let first = record
    .get("first_wait_at")?
    .as_str()
    .and_then(parse_iso)
    .map(epoch_millis)?;
  let last = record
    .get("last_attempt_at")?
    .as_str()
    .and_then(parse_iso)
    .map(epoch_millis)?;
  if now_ms.saturating_sub(last) > STALE.as_millis().try_into().unwrap_or(u64::MAX) {
    return None;
  }
  Some(json!({
    "key": record.get("key")?.as_str()?,
    "epic": record.get("epic")?.as_str()?,
    "worktree": record.get("worktree")?.as_str()?,
    "age_ms": now_ms.saturating_sub(first),
  }))
}

#[cfg(test)]
#[path = "tests/waiting_jobs_test.rs"]
mod tests;
