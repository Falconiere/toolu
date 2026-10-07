//! Load registry, graph, issue and status snapshots, then the journal tail.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde_json::Value;

use crate::disk::read_value;
use crate::journal;
use crate::logic::{arm_ready, ensure_issue};
use crate::model::{Cycle, World};
use crate::paths::Paths;
use crate::recover::{observe, reconcile_stages};

/// Issues, pause and watch deadlines for one resource root.
///
/// # Errors
/// A snapshot or the journal cannot be read.
pub(crate) fn load(paths: &Paths, now: SystemTime) -> Result<World, String> {
  let now_ms = toolu_state::time::epoch_millis(now);
  let mut world = World::new(now_ms);
  apply_pause(&mut world, &read_value(&paths.pause())?);
  apply_watch(&mut world, &read_value(&paths.watch())?);
  let stages = load_registry(&mut world, &read_value(&paths.registry())?)?;
  for record in journal::tail(&paths.journal_dir(), now)? {
    observe(&mut world, &record);
  }
  overlay_stages(&mut world, &stages);
  reconcile_stages(&mut world);
  arm_ready(&mut world);
  Ok(world)
}

fn apply_pause(world: &mut World, pause: &Value) {
  world.paused_all = pause.get("all").and_then(Value::as_bool).unwrap_or(false);
  let Some(epics) = pause.get("epics").and_then(Value::as_array) else {
    return;
  };
  for epic in epics {
    if let Some(epic) = epic.as_str() {
      world.paused.insert(epic.to_owned());
    }
  }
}

fn apply_watch(world: &mut World, watch: &Value) {
  if watch.get("version").and_then(Value::as_u64) != Some(1) {
    return;
  }
  if let Some(failures) = watch.get("herdrFailures").and_then(Value::as_u64) {
    world.herdr_failures = u32::try_from(failures).unwrap_or(u32::MAX);
  }
  if let Some(retry) = watch.get("herdrRetryAt").and_then(Value::as_u64) {
    world.herdr_retry_at_ms = retry;
  }
  if let Some(next) = watch.get("nextCheckpointAt").and_then(Value::as_u64) {
    world.checkpoint_at_ms = next;
  }
}

fn load_registry(world: &mut World, registry: &Value) -> Result<BTreeMap<String, String>, String> {
  let mut stages = BTreeMap::new();
  let Some(epics) = registry.get("epics").and_then(Value::as_array) else {
    return Ok(stages);
  };
  for epic in epics {
    let key = epic.get("key").and_then(Value::as_str).unwrap_or("");
    let dir = epic.get("state_dir").and_then(Value::as_str).unwrap_or("");
    if key.is_empty() || dir.is_empty() {
      continue;
    }
    load_epic(world, key, Path::new(dir), &mut stages)?;
  }
  Ok(stages)
}

fn load_epic(
  world: &mut World,
  epic: &str,
  dir: &Path,
  stages: &mut BTreeMap<String, String>,
) -> Result<(), String> {
  let graph = read_value(&dir.join("graph.json"))?;
  if graph_cycle(&graph) && world.cycle == Cycle::Clear {
    world.cycle = Cycle::Open;
  }
  if let Some(limit) = graph.get("max_parallel").and_then(Value::as_u64) {
    world.max_parallel = u32::try_from(limit).unwrap_or(1).max(1);
  }
  let Some(issues) = graph.get("issues").and_then(Value::as_array) else {
    return Ok(());
  };
  for item in issues {
    load_issue(world, epic, dir, item, stages)?;
  }
  Ok(())
}

fn graph_cycle(graph: &Value) -> bool {
  match graph.get("cycle") {
    Some(Value::Array(items)) => !items.is_empty(),
    Some(Value::String(text)) => !text.is_empty(),
    Some(Value::Bool(open)) => *open,
    _ => false,
  }
}

fn load_issue(
  world: &mut World,
  epic: &str,
  dir: &Path,
  item: &Value,
  stages: &mut BTreeMap<String, String>,
) -> Result<(), String> {
  let Some(key) = item
    .get("key")
    .and_then(Value::as_str)
    .filter(|key| !key.is_empty())
  else {
    return Ok(());
  };
  let state_dir = dir.display().to_string();
  let issue_path = dir.join("issues").join(format!("{key}.json"));
  let status_path = dir.join("status").join(format!("{key}.json"));
  let issue_doc = read_value(&issue_path)?;
  let status = read_value(&status_path)?;
  let issue = ensure_issue(world, key, epic, &state_dir);
  issue.blockers = strings(item.get("open_blockers"));
  if let Some(stage) = issue_doc.get("stage").and_then(Value::as_str) {
    stages.insert(key.to_owned(), stage.to_owned());
  }
  if let Some(tree) = issue_doc.get("worktree").and_then(Value::as_str) {
    issue.worktree = Some(tree.to_owned());
  }
  if let Some(phase) = status.get("phase").and_then(Value::as_str) {
    phase.clone_into(&mut issue.phase);
  }
  issue.pr = status.get("pr").and_then(Value::as_u64);
  if let Some(note) = status.get("note").and_then(Value::as_str) {
    note.clone_into(&mut issue.note);
  }
  Ok(())
}

fn strings(value: Option<&Value>) -> Vec<String> {
  value
    .and_then(Value::as_array)
    .map(|items| {
      items
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
    })
    .unwrap_or_default()
}

fn overlay_stages(world: &mut World, stages: &BTreeMap<String, String>) {
  for (key, stage) in stages {
    if let Some(issue) = world.issues.get_mut(key)
      && rank(stage) >= rank(&issue.stage)
    {
      issue.stage.clone_from(stage);
    }
  }
}

fn rank(stage: &str) -> u8 {
  match stage {
    "" => 0,
    "starting" => 1,
    "running" => 2,
    "cleaning" => 3,
    _ => 4,
  }
}

/// Add epics that were registered after this process opened.
///
/// # Errors
/// A new epic's graph or status file cannot be read.
pub(crate) fn adopt_new_epics(world: &mut World, paths: &Paths) -> Result<(), String> {
  let registry = read_value(&paths.registry())?;
  let Some(epics) = registry.get("epics").and_then(Value::as_array) else {
    return Ok(());
  };
  let mut stages = BTreeMap::new();
  for epic in epics {
    let key = epic.get("key").and_then(Value::as_str).unwrap_or("");
    let dir = epic.get("state_dir").and_then(Value::as_str).unwrap_or("");
    let known = world.issues.values().any(|issue| issue.epic == key);
    if key.is_empty() || dir.is_empty() || known {
      continue;
    }
    load_epic(world, key, Path::new(dir), &mut stages)?;
  }
  overlay_stages(world, &stages);
  Ok(())
}

/// Issue snapshot path.
pub(crate) fn issue_path(state_dir: &str, key: &str) -> PathBuf {
  Path::new(state_dir)
    .join("issues")
    .join(format!("{key}.json"))
}

/// Status snapshot path.
pub(crate) fn status_path(state_dir: &str, key: &str) -> PathBuf {
  Path::new(state_dir)
    .join("status")
    .join(format!("{key}.json"))
}

#[cfg(test)]
#[path = "tests/snapshot_test.rs"]
mod tests;
