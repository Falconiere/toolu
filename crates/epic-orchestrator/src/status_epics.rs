//! Safe summaries of registered epic graphs and their live issues.

use std::collections::BTreeMap;
use std::path::PathBuf;

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

/// One epic's safe graph, live issues and current ready queue.
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
  let (nodes, waves, queue) = project_graph(&graph, world);
  let issues: Vec<Value> = world
    .issues
    .values()
    .filter(|issue| issue.epic == *key)
    .map(|issue| {
      json!({
        "key": issue.key,
        "stage": issue.stage,
        "phase": issue.phase,
        "pr": issue.pr,
        "host": issue.kind,
      })
    })
    .collect();
  Ok(Some(json!({
    "schema": 1,
    "key": key,
    "paused": world.paused_all || world.paused.contains(key),
    "graph": {"issues": nodes, "waves": waves},
    "issues": issues,
    "queue": queue,
  })))
}

/// Ready work grouped by repository in stable epic and graph order.
///
/// # Errors
/// A registered graph cannot be read.
pub(crate) fn queue(world: &World, paths: &Paths) -> Result<Value, String> {
  let mut by_repo = BTreeMap::<String, Vec<String>>::new();
  for (_epic, dir) in registered(paths)? {
    let graph = read_value(&dir.join("graph.json"))?;
    let (_nodes, _waves, queued) = project_graph(&graph, world);
    for item in queued {
      let Some(repo) = item.get("repo").and_then(Value::as_str) else {
        continue;
      };
      let Some(key) = item.get("key").and_then(Value::as_str) else {
        continue;
      };
      by_repo
        .entry(repo.to_owned())
        .or_default()
        .push(key.to_owned());
    }
  }
  let repositories: Vec<Value> = by_repo
    .into_iter()
    .map(|(repo, keys)| {
      let items: Vec<Value> = keys
        .into_iter()
        .enumerate()
        .map(|(index, key)| json!({"key": key, "position": index + 1}))
        .collect();
      json!({"repo": repo, "items": items})
    })
    .collect();
  Ok(json!({"schema": 1, "repositories": repositories}))
}

fn project_graph(graph: &Value, world: &World) -> (Vec<Value>, Vec<Value>, Vec<Value>) {
  let mut nodes = Vec::new();
  let mut waves = BTreeMap::<u64, Vec<String>>::new();
  let mut queue = Vec::new();
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
    if world
      .issues
      .get(key)
      .is_some_and(|issue| issue.phase == "ready")
    {
      queue.push(json!({"key": key, "repo": row.get("repo").and_then(Value::as_str)}));
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
  (nodes, waves, queue)
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
