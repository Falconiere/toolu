//! Resolve a saved terminal identity before sending input.

use crate::paths::Paths;
use serde_json::Value;
use std::path::Path;
use toolu_mux::hc::{Client, Error, require_owner};
use toolu_runtime::env::Env;

fn failure(message: impl Into<String>) -> Error {
  Error::new("identity_mismatch", message)
}

fn all_records(paths: &Paths) -> Result<Vec<Value>, Error> {
  let registry = crate::registry::read(paths).map_err(failure)?;
  let mut records = Vec::new();
  let epics = registry
    .get("epics")
    .and_then(Value::as_array)
    .ok_or_else(|| failure("invalid epic registry"))?;
  for epic in epics {
    let dir = epic
      .get("state_dir")
      .and_then(Value::as_str)
      .ok_or_else(|| failure("invalid epic state directory"))?;
    let entries = std::fs::read_dir(Path::new(dir).join("issues"))
      .map_err(|e| failure(format!("cannot inspect issue records: {e}")))?;
    for entry in entries {
      let path = entry.map_err(|e| failure(e.to_string()))?.path();
      if path.extension().and_then(|s| s.to_str()) == Some("json") {
        records.push(crate::disk::read_value(&path).map_err(failure)?);
      }
    }
  }
  Ok(records)
}

fn records(paths: &Paths, target: &str) -> Result<Value, Error> {
  let mut found = all_records(paths)?
    .into_iter()
    .filter(|value| value.get("terminal_address").and_then(Value::as_str) == Some(target));
  let value = found
    .next()
    .ok_or_else(|| failure("no registered issue owns this terminal"))?;
  if found.next().is_some() {
    return Err(failure("multiple records claim this terminal"));
  }
  Ok(value)
}

pub(super) fn tracked(paths: &Paths, env: &Env, client: &Client<'_>) -> Result<Vec<Value>, Error> {
  let mut terminals = Vec::new();
  for record in all_records(paths)? {
    if record.get("backend").and_then(Value::as_str) != Some("herdr-app") {
      continue;
    }
    if !matches!(
      record.get("stage").and_then(Value::as_str),
      Some("starting" | "uncertain" | "replacing" | "running" | "awaiting_merge")
    ) {
      continue;
    }
    let address = record
      .get("terminal_address")
      .and_then(Value::as_str)
      .filter(|s| !s.is_empty())
      .ok_or_else(|| failure("active hc record has no address"))?;
    let mut terminal = client.get(address)?;
    matches(&record, &terminal, env)?;
    terminal
      .as_object_mut()
      .ok_or_else(|| failure("invalid terminal"))?
      .insert("address".into(), Value::String(address.to_owned()));
    terminals.push(terminal);
  }
  Ok(terminals)
}

fn matches(record: &Value, terminal: &Value, env: &Env) -> Result<(), Error> {
  let get = |key: &str| {
    record
      .get(key)
      .and_then(Value::as_str)
      .filter(|s| !s.is_empty())
      .ok_or_else(|| failure(format!("missing {key}")))
  };
  if get("hc_session")? != env.get("HC_SESSION").unwrap_or("default") {
    return Err(failure("hc session differs"));
  }
  let owner = get("terminal_owner")?;
  if owner.strip_prefix("toolu.owner=") != Some(get("lease_token")?) {
    return Err(failure("hc owner differs from lease"));
  }
  require_owner(terminal, owner)?;
  let local = get("worktree")?;
  let cwd = if record.get("controller_review").and_then(Value::as_bool) == Some(true) {
    local
  } else {
    record
      .get("remote_worktree")
      .and_then(Value::as_str)
      .unwrap_or(local)
  };
  if terminal.get("cwd").and_then(Value::as_str) != Some(cwd) {
    return Err(failure("hc worktree differs"));
  }
  Ok(())
}

pub(super) fn verify(paths: &Paths, env: &Env, target: &str) -> Result<(), Error> {
  let record = records(paths, target)?;
  let terminal = Client::new(env).get(target)?;
  matches(&record, &terminal, env)
}

#[cfg(test)]
#[path = "tests/identity_test.rs"]
mod tests;
