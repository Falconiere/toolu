//! The slot's state file (version 2): loading with the shipped checks, atomic
//! JSON writes, the slot name and the sibling file paths.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};
use toolu_runtime::env::Env;

use crate::error::{Failure, Result};
use crate::lock::SlotLock;

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

fn retired_group(group: &Value) -> bool {
  let retired = [
    "hcName",
    "hcAddress",
    "hcOwner",
    "hcSession",
    "hcCommand",
    "hcSendStarted",
    "hcSendConfirmed",
    "relayRequest",
    "remote",
    "remoteImported",
    "remoteExportStarted",
  ]
  .iter()
  .any(|key| group.get(key).is_some_and(|value| !value.is_null()));
  retired
    || group
      .get("agent")
      .and_then(Value::as_str)
      .is_some_and(|target| target.starts_with("hc:") || target.starts_with("relay:"))
}

/// Refuse retired worker state before reducing it or acting on its identities.
pub(crate) fn require_local(path: &Path, value: &Value) -> Result<()> {
  let has = |pointer| value.pointer(pointer).is_some_and(|value| !value.is_null());
  let retired = value.get("relayEnabled") == Some(&Value::Bool(true))
    || has("/relayMachine")
    || value
      .pointer("/herdrWorktree/multiplexer")
      .and_then(Value::as_str)
      == Some("herdr-app")
    || has("/herdrWorktree/remote")
    || has("/herdrWorktree/remoteRemoval")
    || has("/fixer/remoteRemoval")
    || value.get("remoteHistory").is_some_and(|history| {
      !history.is_null() && history.as_array().is_none_or(|entries| !entries.is_empty())
    })
    || value
      .pointer("/fixer/groups")
      .and_then(Value::as_array)
      .is_some_and(|groups| groups.iter().any(retired_group));
  if retired {
    return Err(
      Failure::new(
        "unsupported_worker_state",
        format!(
          "unsupported hc/relay worker state in {}; use a pre-removal Toolu binary to settle or recover it",
          path.display()
        ),
      )
      .with("source", "state"),
    );
  }
  Ok(())
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
  require_local(path, &value)?;
  Ok(value)
}

/// Hold the slot lock, then load the state behind it.
pub(crate) fn open(path: &Path, env: &Env) -> Result<(SlotLock, Value)> {
  let lock = SlotLock::acquire(path, env)?;
  Ok((lock, load(path)?))
}

/// The object at `key` of `map`, replacing anything that is not an object.
fn object<'v>(map: &'v mut Map<String, Value>, key: &str) -> Result<&'v mut Map<String, Value>> {
  let value = map.entry(key).or_insert_with(|| json!({}));
  if !value.is_object() {
    *value = json!({});
  }
  value
    .as_object_mut()
    .ok_or_else(|| malformed(format!("state {key} is not an object")))
}

/// `actions.<name>` of a loaded state, created when absent.
pub(crate) fn actions<'v>(state: &'v mut Value, name: &str) -> Result<&'v mut Map<String, Value>> {
  let root = state
    .as_object_mut()
    .ok_or_else(|| malformed("state is not an object".into()))?;
  object(object(root, "actions")?, name)
}

/// The repository, number and recorded head SHA a loaded state names.
pub(crate) fn target(state: &Value) -> (&str, u64, Value) {
  (
    state.get("repo").and_then(Value::as_str).unwrap_or(""),
    state.get("number").and_then(Value::as_u64).unwrap_or(0),
    state.pointer("/pr/headSha").cloned().unwrap_or(Value::Null),
  )
}

/// Write `value` and a newline to `path` atomically.
pub(crate) fn write_json(path: &Path, value: &Value, source: &str) -> Result<()> {
  write_text(path, &json_text(value), source)
}

/// `value` as the text [`write_json`] writes.
pub(crate) fn json_text(value: &Value) -> String {
  format!("{value}\n")
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
