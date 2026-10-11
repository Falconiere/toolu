//! Allow-listed status documents shared by the control socket and HTTP API.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{Value, json};
use toolu_runtime::config::secrets::{self, Secrets};

use crate::disk::read_value;
use crate::model::{Issue, World};
use crate::paths::Paths;
use crate::status_resources::Snapshot;
use crate::{status, status_epics, status_events};

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
    let mut value = issue_document(issue, world, paths, secrets)?;
    if let Some(map) = value.as_object_mut() {
      map.insert("engine".to_owned(), json!("running"));
    }
    return Ok(value);
  }
  if let Some(selector) = selector {
    let mut value = status_epics::detail(world, paths, selector)?
      .ok_or_else(|| format!("unknown epic or issue: {selector}"))?;
    if let Some(map) = value.as_object_mut() {
      map.insert("engine".to_owned(), json!("running"));
    }
    return Ok(secrets::redact_json(&value, secrets));
  }
  let mut overview = status::document(world, true, None);
  let mut counts = BTreeMap::<String, u64>::new();
  for issue in world.issues.values() {
    *counts.entry(issue.stage.clone()).or_default() += 1;
  }
  let attention_count = world
    .attention
    .iter()
    .filter(|item| !item.delivered)
    .count();
  let resources = Snapshot::open(paths, world.now_ms)?;
  if let Some(map) = overview.as_object_mut() {
    map.insert("schema".to_owned(), json!(1));
    map.insert("journal_seq".to_owned(), json!(world.seq));
    map.insert("counts".to_owned(), json!(counts));
    map.insert("attention_count".to_owned(), json!(attention_count));
    map.insert("epics".to_owned(), json!(status_epics::list(world, paths)?));
    map.insert("hosts".to_owned(), resources.hosts());
    map.insert("machine".to_owned(), resources.machine());
    map.insert("jobs".to_owned(), resources.jobs());
  }
  Ok(secrets::redact_json(&overview, secrets))
}

/// A versioned JSON endpoint, or `None` for an unknown path.
///
/// # Errors
/// A registered snapshot, resource state or journal cannot be read.
pub(crate) fn route(
  world: &World,
  paths: &Paths,
  path: &str,
  uptime_ms: u64,
  secrets: &Secrets,
) -> Result<Option<Value>, String> {
  let value = match path {
    "/healthz" => Some(json!({
      "schema": 1, "version": env!("CARGO_PKG_VERSION"),
      "uptime_ms": uptime_ms, "journal_seq": world.seq,
    })),
    "/api/v1/overview" => Some(document(world, paths, None, secrets)?),
    "/api/v1/epics" => Some(json!({"schema": 1, "items": status_epics::list(world, paths)?})),
    "/api/v1/agents" => Some(agents(world)?),
    "/api/v1/hosts" => Some(Snapshot::open(paths, world.now_ms)?.hosts()),
    "/api/v1/machine" => Some(Snapshot::open(paths, world.now_ms)?.machine()),
    "/api/v1/jobs" => Some(Snapshot::open(paths, world.now_ms)?.jobs()),
    "/api/v1/queue" => Some(status_epics::queue(world, paths)?),
    "/api/v1/attention" => Some(attention(world)),
    _ => dynamic_route(world, paths, path, secrets)?,
  };
  Ok(value.map(|value| secrets::redact_json(&value, secrets)))
}

fn dynamic_route(
  world: &World,
  paths: &Paths,
  path: &str,
  secrets: &Secrets,
) -> Result<Option<Value>, String> {
  if let Some(key) = path.strip_prefix("/api/v1/epics/") {
    return status_epics::detail(world, paths, key);
  }
  if let Some(key) = path.strip_prefix("/api/v1/issues/") {
    return world
      .issues
      .get(key)
      .map(|issue| issue_document(issue, world, paths, secrets))
      .transpose();
  }
  Ok(None)
}

fn agents(world: &World) -> Result<Value, String> {
  let mut items = Vec::new();
  for issue in world.issues.values() {
    let meta = read_value(
      &Path::new(&issue.state_dir)
        .join("issues")
        .join(format!("{}.json", issue.key)),
    )?;
    items.push(json!({
      "state": issue.stage,
      "key": issue.key,
      "epic": issue.epic,
      "worktree": field(&meta, "worktree").or(issue.worktree.as_deref()),
      "session_id": field(&meta, "session_id"),
    }));
  }
  Ok(json!({"schema": 1, "items": items}))
}

fn attention(world: &World) -> Value {
  let items: Vec<Value> = world
    .attention
    .iter()
    .filter(|item| !item.delivered)
    .map(|item| {
      let phase_at = world.issues.get(&item.key).map(|issue| issue.phase_at_ms);
      json!({
        "kind": item.kind,
        "key": item.key,
        "epic": item.epic,
        "note": item.note.chars().take(160).collect::<String>(),
        "seq": item.seq,
        "age_ms": phase_at.map(|at| world.now_ms.saturating_sub(at)),
      })
    })
    .collect();
  json!({"schema": 1, "items": items})
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
