//! The slot's state file (version 2): loading with the shipped checks, atomic
//! JSON writes, the slot name and the sibling file paths.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::error::{Failure, Result};

/// `owner/name` as GitHub allows it.
pub(crate) fn valid_repo(repo: &str) -> bool {
  let Some((owner, name)) = repo.split_once('/') else {
    return false;
  };
  let owner_ok = owner
    .chars()
    .next()
    .is_some_and(|first| first.is_ascii_alphanumeric())
    && owner
      .chars()
      .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
  let name_ok = name
    .chars()
    .next()
    .is_some_and(|first| first.is_ascii_alphanumeric() || first == '_')
    && name
      .chars()
      .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'));
  owner_ok && name_ok
}

/// `<owner>-<repo>-<number>`, lowercased.
pub(crate) fn slot(repo: &str, number: u64) -> String {
  format!("{}-{number}", repo.to_lowercase().replace('/', "-"))
}

/// `path` without a trailing `.json`, plus `suffix`.
pub(crate) fn sibling(path: &Path, suffix: &str) -> PathBuf {
  let text = path.to_string_lossy();
  let stem = text.strip_suffix(".json").unwrap_or(&text);
  PathBuf::from(format!("{stem}{suffix}"))
}

fn malformed(message: String) -> Failure {
  Failure::new("state_malformed", message).with("source", "state")
}

/// Read and check a version-2 state file the agent's write verbs act on.
pub(crate) fn load(path: &Path) -> Result<Value> {
  let shown = path.display();
  let text = std::fs::read_to_string(path).ok().ok_or_else(|| {
    malformed(format!(
      "state file not found: {shown} (run toolu babysit tick first)"
    ))
  })?;
  let value: Value = serde_json::from_str(&text)
    .ok()
    .ok_or_else(|| malformed(format!("state file is not valid JSON: {shown}")))?;
  let not_v2 = || {
    malformed(format!(
      "state file is not a version-2 pr-babysit state: {shown}"
    ))
    .with(
      "version",
      value.get("version").cloned().unwrap_or(Value::Null),
    )
  };
  if value.get("version").and_then(Value::as_u64) != Some(2) {
    return Err(not_v2());
  }
  let Some(repo) = value.get("repo").and_then(Value::as_str) else {
    return Err(not_v2());
  };
  if !value.get("number").is_some_and(Value::is_number) {
    return Err(not_v2());
  }
  if !valid_repo(repo) {
    return Err(malformed(format!(
      "state file repo is not owner/name: {repo}"
    )));
  }
  if value.get("number").and_then(Value::as_u64).is_none() {
    return Err(malformed(format!(
      "state file number is not an integer: {}",
      value.get("number").unwrap_or(&Value::Null)
    )));
  }
  Ok(value)
}

/// Write `value` and a newline to `path` atomically.
pub(crate) fn write_json(path: &Path, value: &Value, source: &str) -> Result<()> {
  write_text(path, &format!("{value}\n"), source)
}

/// Write `text` to `path` atomically.
pub(crate) fn write_text(path: &Path, text: &str, source: &str) -> Result<()> {
  if toolu_runtime::atomic::write_atomic(path, text) {
    Ok(())
  } else {
    Err(
      Failure::new(
        "invalid_json",
        format!("could not write {}", path.display()),
      )
      .with("source", source),
    )
  }
}

#[cfg(test)]
#[path = "tests/state_test.rs"]
mod tests;
