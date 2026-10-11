//! Allow-listed status documents shared by the control socket and HTTP API.

use std::path::Path;

use serde_json::{Value, json};
use toolu_runtime::config::secrets::{self, Secrets};

use crate::disk::read_value;
use crate::model::{Issue, World};
use crate::paths::Paths;
use crate::{status, status_events};

/// Status for the engine, one epic or one issue.
///
/// # Errors
/// A registered snapshot or journal cannot be read.
pub(crate) fn document(
  world: &World,
  paths: &Paths,
  selector: Option<&str>,
  secrets: &Secrets,
) -> Result<Value, String> {
  if let Some(issue) = selector.and_then(|key| world.issues.get(key)) {
    return issue_document(issue, world, paths, secrets);
  }
  let mut overview = status::document(world, true, selector);
  if let Some(map) = overview.as_object_mut() {
    map.insert("schema".to_owned(), json!(1));
    map.insert("journal_seq".to_owned(), json!(world.seq));
  }
  Ok(secrets::redact_json(&overview, secrets))
}

fn issue_document(
  issue: &Issue,
  world: &World,
  paths: &Paths,
  secrets: &Secrets,
) -> Result<Value, String> {
  let dir = Path::new(&issue.state_dir);
  let meta = read_value(&dir.join("issues").join(format!("{}.json", issue.key)))?;
  let status = read_value(&dir.join("status").join(format!("{}.json", issue.key)))?;
  let route = read_value(&dir.join("routes").join(format!("{}.json", issue.key)))?;
  let events = issue_events(issue, world, paths, secrets)?;
  let history = phase_history(&status);
  let host = field(&route, "host").or_else(|| field(&meta, "kind"));
  let model = field(&route, "model").or_else(|| field(&meta, "model"));
  let effort = field(&route, "effort").or_else(|| field(&meta, "effort"));
  let last_gate = events.iter().rev().find_map(|row| {
    (row.get("kind").and_then(Value::as_str) == Some("gate"))
      .then(|| row.get("name").cloned())
      .flatten()
  });
  let value = json!({
    "schema": 1,
    "key": issue.key,
    "epic": issue.epic,
    "stage": issue.stage,
    "phase": issue.phase,
    "phase_age_ms": world.now_ms.saturating_sub(issue.phase_at_ms),
    "host": host,
    "model": model,
    "effort": effort,
    "pr": issue.pr,
    "last_gate_verdict": last_gate,
    "queue_position": null,
    "launches": meta.get("launches").and_then(Value::as_u64).unwrap_or(u64::from(issue.launches)),
    "phase_history": history,
    "recent_events": events,
  });
  Ok(secrets::redact_json(&value, secrets))
}

fn issue_events(
  issue: &Issue,
  world: &World,
  paths: &Paths,
  secrets: &Secrets,
) -> Result<Vec<Value>, String> {
  let page = status_events::page(
    &paths.journal_dir(),
    world.seq.saturating_sub(200),
    200,
    secrets,
  )?;
  Ok(
    page
      .get("events")
      .and_then(Value::as_array)
      .into_iter()
      .flatten()
      .filter(|row| row.get("key").and_then(Value::as_str) == Some(&issue.key))
      .cloned()
      .collect(),
  )
}

fn field<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
  value.get(key).and_then(Value::as_str)
}

fn phase_history(status: &Value) -> Vec<Value> {
  status
    .get("history")
    .and_then(Value::as_array)
    .into_iter()
    .flatten()
    .map(|row| {
      json!({
        "phase": field(row, "phase").unwrap_or(""),
        "at": field(row, "at"),
        "note": field(row, "note").unwrap_or("").chars().take(160).collect::<String>(),
      })
    })
    .collect()
}

#[cfg(test)]
#[path = "tests/status_api_test.rs"]
mod tests;
