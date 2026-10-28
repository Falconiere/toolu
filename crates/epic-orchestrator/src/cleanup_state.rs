//! Conservative read-only eligibility and removal of registered in-memory state.

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::Value;

use crate::disk::read_value;
use crate::journal::Record;
use crate::model::{Issue, Step, World};
use crate::paths::Paths;
use crate::server::Engine;

/// A reason local activity prevents unregistering this epic.
pub(crate) fn safety(world: &World, key: &str) -> Option<&'static str> {
  if let Some(epic) = world.epics.get(key) {
    if epic.close_blocked {
      return Some("failed or uncertain closure");
    }
    if epic.pending.is_some() || epic.batch.iter().any(|member| member.outcome.is_none()) {
      return Some("pending epic action");
    }
  }
  for issue in world.issues.values().filter(|issue| issue.epic == key) {
    if issue.pending.is_some()
      || issue.deferred.is_some()
      || issue.relaunch.is_some()
      || issue.queue.disarm_due
      || issue.queue.auto_armed
      || issue.queue.held.is_some()
    {
      return Some("pending worker action");
    }
    if matches!(issue.stage.as_str(), "cleaning" | "cleanup-incomplete") {
      return Some("unfinished teardown");
    }
    if crate::graph::occupies_slot(Some(&issue.stage))
      || matches!(issue.agent_status.as_str(), "working" | "blocked" | "idle")
    {
      return Some("active worker");
    }
  }
  None
}

/// Inspect disk state and retained journal records without arming or running actions.
pub(crate) fn inspect(
  paths: &Paths,
  key: &str,
  dir: &str,
  records: &[Record],
) -> Result<&'static str, String> {
  resource_safety(paths, dir)?;
  let mut world = World::new(0);
  world
    .epics
    .insert(key.to_owned(), crate::epics::Epic::new(key, dir, 0, None));
  let missing = match std::fs::metadata(dir) {
    Ok(meta) if meta.is_dir() => false,
    Ok(_) => return Err("state path is not a directory".to_owned()),
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => true,
    Err(err) => return Err(format!("unreadable state directory: {err}")),
  };
  if !missing {
    load(&mut world, key, Path::new(dir))?;
  }
  replay(&mut world, key, records);
  journal_safety(key, &world, records)?;
  if let Some(reason) = safety(&world, key) {
    return Err(reason.to_owned());
  }
  pending_spool(paths, key, dir)?;
  let epic = world
    .epics
    .get(key)
    .ok_or_else(|| "missing epic state".to_owned())?;
  if missing {
    Ok("missing state directory")
  } else if epic.closed {
    Ok("locally closed")
  } else if epic.complete {
    Ok("locally complete")
  } else {
    Err("unfinished epic".to_owned())
  }
}

fn replay(world: &mut World, key: &str, records: &[Record]) {
  let stages = world
    .issues
    .iter()
    .map(|(key, issue)| (key.clone(), issue.stage.clone()))
    .collect();
  for record in records {
    if record.key == key || record.epic == key || world.issues.contains_key(&record.key) {
      crate::recover::observe(world, record);
    }
  }
  crate::snapshot::overlay_stages(world, &stages);
}

fn load(world: &mut World, key: &str, dir: &Path) -> Result<(), String> {
  let graph = crate::graph::read_graph(&dir.join("graph.json"))?;
  let value = serde_json::to_value(&graph).map_err(|err| err.to_string())?;
  crate::epics::load_epic_state(world, key, &dir.display().to_string(), &value);
  for item in &graph.issues {
    let stage = item.stage.as_deref().unwrap_or("");
    if !stage.is_empty() && !crate::finish::record::STAGES.contains(&stage) {
      return Err(format!("unrecognized graph stage for {}", item.key));
    }
    let mut issue = Issue::blank(&item.key, key, &dir.display().to_string(), 0);
    issue.stage = item.stage.clone().unwrap_or_default();
    world.issues.insert(item.key.clone(), issue);
  }
  for path in json_files(&dir.join("issues"))? {
    let issue_key = path
      .file_stem()
      .and_then(|name| name.to_str())
      .unwrap_or("");
    let map = crate::finish::record::read_issue_record(&path, issue_key)?;
    let stage = map.get("stage").and_then(Value::as_str).unwrap_or("");
    if !matches!(stage, "merged" | "abandoned") {
      return Err(format!(
        "active worker or unfinished teardown: {}",
        path.display()
      ));
    }
    if !crate::finish::record::never_started(&map)
      && !crate::finish::record::terminal_cleanup_complete(&map)
    {
      return Err(format!("unfinished teardown: {}", path.display()));
    }
    let mut issue = Issue::blank(issue_key, key, &dir.display().to_string(), 0);
    stage.clone_into(&mut issue.stage);
    crate::queue_fields::read(&mut issue, &Value::Object(map));
    world.issues.insert(issue_key.to_owned(), issue);
  }
  for path in json_files(&dir.join("status"))? {
    validate_status(world, &path)?;
  }
  Ok(())
}

fn validate_status(world: &World, path: &Path) -> Result<(), String> {
  let status = read_value(path)?;
  if !status.get("phase").is_some_and(Value::is_string) {
    return Err(format!("invalid status {}", path.display()));
  }
  let key = path.file_stem().and_then(|key| key.to_str()).unwrap_or("");
  if !world.issues.get(key).is_some_and(crate::refill::done) {
    return Err(format!(
      "worker status without a settled record: {}",
      path.display()
    ));
  }
  Ok(())
}

fn journal_safety(key: &str, world: &World, records: &[Record]) -> Result<(), String> {
  let mut pending = std::collections::BTreeMap::new();
  for record in records.iter().filter(|record| {
    record.key == key || record.epic == key || world.issues.contains_key(&record.key)
  }) {
    if let Some(action) = record.name.strip_suffix("-intent") {
      pending.insert((&record.key, action), &record.token);
    }
    if let Some(action) = record.name.strip_suffix("-done")
      && pending.get(&(&record.key, action)) == Some(&&record.token)
    {
      pending.remove(&(&record.key, action));
    }
  }
  if pending.is_empty() {
    Ok(())
  } else {
    Err("pending journal action".to_owned())
  }
}

fn json_files(dir: &Path) -> Result<Vec<std::path::PathBuf>, String> {
  let entries = match std::fs::read_dir(dir) {
    Ok(entries) => entries,
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
    Err(err) => return Err(format!("{}: {err}", dir.display())),
  };
  let mut paths = Vec::new();
  for entry in entries {
    let path = entry.map_err(|err| err.to_string())?.path();
    if path.extension().is_some_and(|ext| ext == "json") {
      paths.push(path);
    }
  }
  Ok(paths)
}

fn pending_spool(paths: &Paths, key: &str, dir: &str) -> Result<(), String> {
  for path in json_files(&paths.spool())? {
    let request = read_value(&path)?;
    if !request.is_object()
      || request.get("epic").and_then(Value::as_str) == Some(key)
      || request.get("state_dir").and_then(Value::as_str) == Some(dir)
    {
      return Err("pending spooled request".to_owned());
    }
  }
  Ok(())
}

fn resource_safety(paths: &Paths, dir: &str) -> Result<(), String> {
  use toolu_engine::resources::store::{leases, read_resource_state};
  use toolu_runtime::json::ordered::Ordered;
  let state = read_resource_state(&paths.root)?;
  if leases(&state).iter().any(
    |lease| matches!(lease.get("stateDir"), Some(Ordered::String(state_dir)) if state_dir == dir),
  ) {
    return Err("worker or job still owns a resource lease".to_owned());
  }
  Ok(())
}

/// An executing or queued call keeps its epic registered.
pub(crate) fn running(engine: &Engine, epics: &BTreeSet<String>) -> Option<&'static str> {
  let mut keys = epics.clone();
  keys.extend(
    engine
      .world
      .issues
      .values()
      .filter(|issue| epics.contains(&issue.epic))
      .map(|issue| issue.key.clone()),
  );
  if engine
    .running
    .as_ref()
    .is_some_and(|call| keys.contains(&call.key))
    || engine
      .parked
      .iter()
      .chain(&engine.world.outbox)
      .any(|step| matches!(step, Step::Call { key, .. } if keys.contains(key)))
  {
    Some("pending engine action")
  } else {
    None
  }
}

/// Remove all live scheduling and presentation state after the registry commit.
pub(crate) fn prune(engine: &mut Engine, epics: &BTreeSet<String>) {
  let world = &mut engine.world;
  let mut keys = epics.clone();
  keys.extend(
    world
      .issues
      .values()
      .filter(|issue| epics.contains(&issue.epic))
      .map(|issue| issue.key.clone()),
  );
  let tokens: BTreeSet<String> = world
    .attention
    .iter()
    .filter(|item| keys.contains(&item.key) || epics.contains(&item.epic))
    .map(|item| item.token.clone())
    .collect();
  let trees: BTreeSet<String> = world
    .issues
    .values()
    .filter(|issue| keys.contains(&issue.key))
    .filter_map(|issue| issue.worktree.clone())
    .collect();
  world.attention.retain(|item| !tokens.contains(&item.token));
  world.epics.retain(|key, _| !epics.contains(key));
  world.epic_refs.retain(|key, _| !epics.contains(key));
  world.issues.retain(|key, _| !keys.contains(key));
  world.checkpointed.retain(|tree| {
    !trees.contains(tree)
      || world
        .issues
        .values()
        .any(|issue| issue.worktree.as_ref() == Some(tree))
  });
  world.paused.retain(|key| !epics.contains(key));
  world.prompt_queue.retain(|key, _| !keys.contains(key));
  world.relayed_at_ms.retain(|key, _| !keys.contains(key));
  world.outbox.retain(|step| keep(step, &keys, &tokens));
  engine.parked.retain(|step| keep(step, &keys, &tokens));
  crate::watch::sync(world);
}

fn keep(step: &Step, keys: &BTreeSet<String>, tokens: &BTreeSet<String>) -> bool {
  match step {
    Step::Call { key, .. }
    | Step::Issue { key }
    | Step::Status { key }
    | Step::Prompt { key, .. } => !keys.contains(key),
    Step::Journal(record) => !keys.contains(&record.key) && !keys.contains(&record.epic),
    Step::Notify { token, .. } => !tokens.contains(token),
  }
}

#[cfg(test)]
#[path = "tests/cleanup_state_test.rs"]
mod tests;
