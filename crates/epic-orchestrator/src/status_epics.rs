//! Safe summaries of registered epic graphs and their live issues.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::disk::read_value;
use crate::model::{Issue, World};
use crate::paths::Paths;

/// Registered epic keys and their state directories.
///
/// # Errors
/// The registry cannot be read.
pub(crate) fn registered(paths: &Paths) -> Result<Vec<(String, PathBuf)>, String> {
  let value = read_value(&paths.registry())?;
  Ok(
    value
      .get("epics")
      .and_then(Value::as_array)
      .into_iter()
      .flatten()
      .filter_map(|row| {
        let key = row.get("key")?.as_str()?.to_owned();
        let dir = PathBuf::from(row.get("state_dir")?.as_str()?);
        (!key.is_empty()).then_some((key, dir))
      })
      .collect(),
  )
}

/// All registered epic summaries, including an empty epic.
///
/// # Errors
/// The registry cannot be read.
pub(crate) fn list(world: &World, paths: &Paths) -> Result<Vec<Value>, String> {
  Ok(
    registered(paths)?
      .iter()
      .map(|(key, _)| summary(world, key))
      .collect(),
  )
}

fn summary(world: &World, key: &str) -> Value {
  let issues: Vec<&Issue> = world
    .issues
    .values()
    .filter(|issue| issue.epic == key)
    .collect();
  let mut counts = BTreeMap::<String, u64>::new();
  for issue in issues {
    *counts.entry(issue.stage.clone()).or_default() += 1;
  }
  let attention = world
    .attention
    .iter()
    .filter(|item| item.epic == key)
    .count();
  json!({
    "key": key,
    "paused": world.paused_all || world.paused.contains(key),
    "counts": counts,
    "attention_count": attention,
  })
}

/// One epic's safe graph, live issues and its merge-queue entries.
///
/// # Errors
/// The registry or graph cannot be read.
pub(crate) fn detail(
  world: &World,
  paths: &Paths,
  selector: &str,
) -> Result<Option<Value>, String> {
  let registered = registered(paths)?;
  let Some((key, dir)) = registered
    .iter()
    .find(|(key, _)| key == selector || key.ends_with(&format!("-{selector}")))
  else {
    return Ok(None);
  };
  let graph = read_value(&dir.join("graph.json"))?;
  let (nodes, waves) = project_graph(&graph);
  let issues = world
    .issues
    .values()
    .filter(|issue| issue.epic == *key)
    .map(issue_row)
    .collect::<Vec<_>>();
  Ok(Some(json!({
    "schema": 1,
    "key": key,
    "paused": world.paused_all || world.paused.contains(key),
    "graph": {"issues": nodes, "waves": waves},
    "issues": issues,
    "queue": crate::queue::document(world, Some(key)).get("repositories").cloned(),
  })))
}

fn issue_row(issue: &Issue) -> Value {
  // One unreadable worker record only loses that row's plan, not the board.
  let meta = read_value(
    &Path::new(&issue.state_dir)
      .join("issues")
      .join(format!("{}.json", issue.key)),
  )
  .unwrap_or(Value::Null);
  let (worktree, host) = crate::status_plan::locate(issue, &meta);
  json!({
    "key": issue.key,
    "stage": issue.stage,
    "phase": issue.phase,
    "pr": issue.pr,
    "host": issue.kind,
    "agent_status": issue.agent_status,
    "agent_status_at": agent_status_at(issue),
    "plan_summary": crate::status_plan::plan_summary(worktree, host),
  })
}

/// When the issue's agent status last changed, as ISO UTC; `None` before any change.
pub(crate) fn agent_status_at(issue: &Issue) -> Option<String> {
  (issue.agent_status_at_ms > 0).then(|| {
    toolu_state::time::iso_millis(
      std::time::UNIX_EPOCH + std::time::Duration::from_millis(issue.agent_status_at_ms),
    )
  })
}

fn project_graph(graph: &Value) -> (Vec<Value>, Vec<Value>) {
  let mut nodes = Vec::new();
  let mut waves = BTreeMap::<u64, Vec<String>>::new();
  for row in graph
    .get("issues")
    .and_then(Value::as_array)
    .into_iter()
    .flatten()
  {
    let Some(key) = row.get("key").and_then(Value::as_str) else {
      continue;
    };
    let wave = row.get("wave").and_then(Value::as_u64);
    if let Some(wave) = wave {
      waves.entry(wave).or_default().push(key.to_owned());
    }
    nodes.push(json!({
      "key": key,
      "title": row.get("title").and_then(Value::as_str),
      "wave": wave,
      "open_blockers": safe_strings(row.get("open_blockers")),
    }));
  }
  let waves = waves
    .into_iter()
    .map(|(wave, keys)| json!({"wave": wave, "issues": keys}))
    .collect();
  (nodes, waves)
}

fn safe_strings(value: Option<&Value>) -> Vec<&str> {
  value
    .and_then(Value::as_array)
    .into_iter()
    .flatten()
    .filter_map(Value::as_str)
    .take(32)
    .collect()
}

#[cfg(test)]
#[path = "tests/status_epics_test.rs"]
mod tests;
