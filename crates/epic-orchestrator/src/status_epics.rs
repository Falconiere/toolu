//! Safe summaries of registered epic graphs and their live issues.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use crate::disk::read_value;
use crate::model::{Issue, World};
use crate::paths::Paths;

const TEXT_LIMIT: usize = 160;

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
  let mut value = json!({
    "key": key,
    "paused": world.paused_all || world.paused.contains(key),
    "counts": counts,
    "attention_count": attention,
  });
  if let Some(map) = value.as_object_mut() {
    map.extend(header(world, key));
  }
  value
}

/// The board header of `key`: title, URL, waves and the live and queued counts.
/// An issue merged, abandoned or closed before launch is done.
fn header(world: &World, key: &str) -> Map<String, Value> {
  let graph = world.epics.get(key).and_then(|epic| epic.graph.as_ref());
  let rows = graph.map_or(&[][..], |graph| graph.issues.as_slice());
  let issues: Vec<&Issue> = world
    .issues
    .values()
    .filter(|issue| issue.epic == key)
    .collect();
  let mut done: BTreeSet<&str> = issues
    .iter()
    .filter(|issue| matches!(issue.stage.as_str(), "merged" | "abandoned"))
    .map(|issue| issue.key.as_str())
    .collect();
  done.extend(
    rows
      .iter()
      .filter(|row| row.status == "done")
      .map(|row| row.key.as_str()),
  );
  let waves: BTreeSet<u64> = rows.iter().filter_map(|row| row.wave).collect();
  let current = rows
    .iter()
    .filter(|row| !done.contains(row.key.as_str()))
    .filter_map(|row| row.wave)
    .min();
  let count = |wanted: &dyn Fn(&Issue) -> bool| issues.iter().filter(|issue| wanted(issue)).count();
  let live = count(&|issue| matches!(issue.stage.as_str(), "running" | "awaiting_merge"));
  let queued = count(&|issue| issue.stage.is_empty() && !done.contains(issue.key.as_str()));
  let header = json!({
    "title": graph.and_then(|graph| cut(&graph.epic.title)),
    "url": graph.and_then(|graph| cut(&graph.epic.url)),
    "waves_total": waves.len(),
    "current_wave": current,
    "live": live,
    "queued": queued,
  });
  header.as_object().cloned().unwrap_or_default()
}

/// `text` cut to 160 characters, or `None` when it is empty.
fn cut(text: &str) -> Option<String> {
  (!text.is_empty()).then(|| text.chars().take(TEXT_LIMIT).collect())
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
    .map(|issue| issue_row(issue, world.now_ms))
    .collect::<Vec<_>>();
  let mut value = json!({
    "schema": 1,
    "key": key,
    "paused": world.paused_all || world.paused.contains(key),
    "graph": {"issues": nodes, "waves": waves},
    "issues": issues,
    "queue": crate::queue::document(world, Some(key)).get("repositories").cloned(),
  });
  if let Some(map) = value.as_object_mut() {
    map.extend(header(world, key));
  }
  Ok(Some(value))
}

fn issue_row(issue: &Issue, now_ms: u64) -> Value {
  // One unreadable worker record only loses that row's fields, not the board.
  let record = |dir: &str| {
    read_value(
      &Path::new(&issue.state_dir)
        .join(dir)
        .join(format!("{}.json", issue.key)),
    )
    .unwrap_or(Value::Null)
  };
  let (meta, route) = (record("issues"), record("routes"));
  let (worktree, host) = crate::status_plan::locate(issue, &meta);
  let (model, effort) = crate::status_api::model_effort(&route, &meta);
  json!({
    "key": issue.key,
    "stage": issue.stage,
    "phase": issue.phase,
    "pr": issue.pr,
    "host": issue.kind,
    "agent_status": issue.agent_status,
    "agent_status_at": agent_status_at(issue),
    "model": model,
    "effort": effort,
    "phase_age_ms": now_ms.saturating_sub(issue.phase_at_ms),
    "hold": hold(issue),
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

/// An issue's coupling hold as `{kind, with, at}`, or `null`; its `score` and
/// `source` stay private.
pub(crate) fn hold(issue: &Issue) -> Value {
  issue.hold.as_ref().map_or(Value::Null, |hold| {
    let text = |key: &str| hold.get(key).and_then(Value::as_str).and_then(cut);
    json!({"kind": text("kind"), "with": text("with"), "at": text("at")})
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
