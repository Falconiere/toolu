//! Durable snapshots of native worker usage for each launch attempt.

use std::path::{Component, Path};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use toolu_runtime::env::Env;

use super::tokens::{self, Agent};
use crate::launch::record::{Record, text};

/// Versioned usage history for an issue.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct History {
  pub(super) schema: u64,
  pub(super) legacy_gap: bool,
  pub(super) attempts: Vec<Attempt>,
}

/// One launch and its latest native usage snapshot.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct Attempt {
  pub(super) id: u64,
  pub(super) host: String,
  pub(super) session: Option<String>,
  pub(super) worktree: String,
  pub(super) started_at: String,
  pub(super) observed_at: Option<String>,
  #[serde(rename = "final")]
  pub(super) finalized: bool,
  pub(super) row: Value,
}

fn valid_session(session: &str) -> bool {
  let mut parts = Path::new(session).components();
  matches!(parts.next(), Some(Component::Normal(_)))
    && parts.next().is_none()
    && !session.contains(['/', '\\', '\0'])
}

fn validate(history: &History, raw: &Value) -> Result<(), String> {
  let invalid = || "invalid usage history".to_owned();
  if history.schema != 1 {
    return Err(invalid());
  }
  let entries = raw
    .get("attempts")
    .and_then(Value::as_array)
    .ok_or_else(invalid)?;
  let mut previous = 0;
  for (attempt, entry) in history.attempts.iter().zip(entries) {
    if attempt.id <= previous
      || !entry.is_object()
      || [
        "id",
        "host",
        "session",
        "worktree",
        "started_at",
        "observed_at",
        "final",
        "row",
      ]
      .iter()
      .any(|key| entry.get(key).is_none())
      || attempt
        .session
        .as_deref()
        .is_some_and(|session| !valid_session(session))
      || !attempt.row.is_object()
    {
      return Err(invalid());
    }
    if let Some(tokens) = attempt.row.get("tokens") {
      let fields = tokens.as_object().ok_or_else(invalid)?;
      if fields.values().any(|value| value.as_u64().is_none()) {
        return Err(invalid());
      }
    }
    previous = attempt.id;
  }
  Ok(())
}

/// Reject malformed history before lifecycle mutations.
///
/// # Errors
/// A saved history violates its schema.
pub(crate) fn validate_record(record: &Record) -> Result<(), String> {
  load(record).map(|_| ())
}

/// Load and validate a saved history.
pub(super) fn load(record: &Record) -> Result<Option<History>, String> {
  let Some(raw) = record.get("usage_history") else {
    return Ok(None);
  };
  let history: History =
    serde_json::from_value(raw.clone()).map_err(|err| format!("invalid usage history: {err}"))?;
  validate(&history, raw)?;
  Ok(Some(history))
}

fn save(record: &mut Record, history: &History) -> Result<(), String> {
  let value = serde_json::to_value(history).map_err(|err| format!("usage history: {err}"))?;
  validate(history, &value)?;
  record.insert("usage_history".to_owned(), value);
  Ok(())
}

fn attempt(id: u64, host: &str, worktree: &str, started_at: &str) -> Attempt {
  Attempt {
    id,
    host: host.to_owned(),
    session: None,
    worktree: worktree.to_owned(),
    started_at: started_at.to_owned(),
    observed_at: None,
    finalized: false,
    row: json!({"status":"unavailable", "reason":"no session id was captured at launch"}),
  }
}

/// Enroll a legacy record and retain its available snapshot.
pub(crate) fn enroll(record: &mut Record, env: &Env) -> Result<(), String> {
  if load(record)?.is_some() {
    return Ok(());
  }
  let launches = record.get("launches").and_then(Value::as_u64).unwrap_or(0);
  let session = text(record, "session_id").map(str::to_owned);
  let existed = launches > 0
    || session.is_some()
    || record.contains_key("stage")
    || record.contains_key("attempt_started_at");
  let mut history = History {
    schema: 1,
    legacy_gap: launches > 1 || (existed && session.is_none()),
    attempts: Vec::new(),
  };
  if existed {
    let mut item = attempt(
      1,
      text(record, "kind").unwrap_or_default(),
      text(record, "worktree").unwrap_or_default(),
      text(record, "attempt_started_at")
        .or_else(|| text(record, "launched_at"))
        .unwrap_or_default(),
    );
    item.session = session;
    refresh(
      &mut item,
      text(record, "key").unwrap_or_default(),
      env,
      false,
    )?;
    history.attempts.push(item);
  }
  save(record, &history)
}

/// Record a launch before starting external work.
pub(crate) fn begin(record: &mut Record, host: &str, started_at: &str) -> Result<(), String> {
  let mut history = load(record)?.ok_or_else(|| "missing usage history".to_owned())?;
  let id = history
    .attempts
    .last()
    .map_or(Some(1), |item| item.id.checked_add(1))
    .ok_or_else(|| "usage history ordinal overflow".to_owned())?;
  history.attempts.push(attempt(
    id,
    host,
    text(record, "worktree").unwrap_or_default(),
    started_at,
  ));
  save(record, &history)
}

/// Capture the current launch identity and provisional usage.
pub(crate) fn attach(record: &mut Record, env: &Env) -> Result<(), String> {
  let mut history = load(record)?.ok_or_else(|| "missing usage history".to_owned())?;
  let item = history
    .attempts
    .last_mut()
    .ok_or_else(|| "missing usage history attempt".to_owned())?;
  item.session = text(record, "session_id").map(str::to_owned);
  text(record, "worktree")
    .unwrap_or_default()
    .clone_into(&mut item.worktree);
  refresh(item, text(record, "key").unwrap_or_default(), env, false)?;
  save(record, &history)
}

/// Refresh the saved identity, optionally marking a terminal snapshot.
pub(crate) fn checkpoint(record: &mut Record, env: &Env, finalized: bool) -> Result<(), String> {
  let mut history = load(record)?.ok_or_else(|| "missing usage history".to_owned())?;
  if let Some(item) = history.attempts.last_mut() {
    refresh(
      item,
      text(record, "key").unwrap_or_default(),
      env,
      finalized,
    )?;
  }
  save(record, &history)
}

/// Refresh an attempt in memory without changing its saved identity.
pub(super) fn refresh(
  item: &mut Attempt,
  key: &str,
  env: &Env,
  finalized: bool,
) -> Result<(), String> {
  if item
    .session
    .as_deref()
    .is_some_and(|session| !valid_session(session))
  {
    return Err("invalid usage history session id".to_owned());
  }
  if item.finalized {
    return Ok(());
  }
  let agent = Agent {
    key: key.to_owned(),
    host: item.host.clone(),
    session: item.session.clone(),
    worktree: item.worktree.clone(),
    launches: 1,
  };
  let row = tokens::row(&agent, env);
  if row.get("status").and_then(Value::as_str) == Some("unavailable") {
    unavailable(item, row)?;
  } else {
    let fields = row
      .get("tokens")
      .and_then(Value::as_object)
      .ok_or_else(|| "invalid usage history tokens".to_owned())?;
    if fields.values().any(|value| value.as_u64().is_none()) {
      return Err("invalid usage history token counters".to_owned());
    }
    item.row = row;
    item.observed_at = Some(toolu_state::time::iso_millis(SystemTime::now()));
    item.finalized = finalized;
  }
  Ok(())
}

fn unavailable(item: &mut Attempt, row: Value) -> Result<(), String> {
  if item.row.get("tokens").is_some() {
    let object = item
      .row
      .as_object_mut()
      .ok_or_else(|| "invalid usage history row".to_owned())?;
    object.insert("status".to_owned(), json!("stale"));
    object.insert(
      "reason".to_owned(),
      row.get("reason").cloned().unwrap_or(Value::Null),
    );
  } else {
    item.row = row;
  }
  item.finalized = false;
  Ok(())
}

#[cfg(test)]
#[path = "tests/usage_test.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/usage_edge_test.rs"]
mod edge_tests;

#[cfg(test)]
#[path = "tests/usage_legacy_test.rs"]
mod legacy_tests;
