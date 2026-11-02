//! Compare-and-swap relay authority and durable retirement.
use super::{Store, config, occupied};
use crate::relay::durable;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Retirement {
  generation: Option<String>,
  successor: String,
}

fn retirement(root: &Path) -> Result<Option<Retirement>, String> {
  match std::fs::read(root.join("retired.json")) {
    Ok(bytes) => serde_json::from_slice(&bytes)
      .map(Some)
      .map_err(|e| format!("invalid retirement: {e}")),
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
    Err(error) => Err(error.to_string()),
  }
}

fn receipt_path(root: &Path, generation: Option<&str>) -> Result<std::path::PathBuf, String> {
  let name = generation.unwrap_or("legacy");
  if !super::super::store::safe(name) {
    return Err("invalid authority generation".into());
  }
  Ok(root.join("retired").join(format!("{name}.json")))
}

fn receipt(root: &Path, generation: Option<&str>) -> Result<Option<Retirement>, String> {
  let path = receipt_path(root, generation)?;
  match std::fs::read(path) {
    Ok(bytes) => {
      let record: Retirement =
        serde_json::from_slice(&bytes).map_err(|e| format!("invalid retirement receipt: {e}"))?;
      if record.generation.as_deref() != generation {
        return Err("retirement receipt generation differs".into());
      }
      Ok(Some(record))
    }
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
    Err(error) => Err(error.to_string()),
  }
}

fn retired_generation(root: &Path, generation: Option<&str>) -> Result<bool, String> {
  Ok(
    receipt(root, generation)?.is_some()
      || retirement(root)?.is_some_and(|r| r.generation.as_deref() == generation),
  )
}

/// Reject mutations unless the marker matches a non-retired authority.
pub(super) fn require(root: &Path) -> Result<(), String> {
  let config = config::load(root)?;
  if retired_generation(root, config.generation.as_deref())? {
    return Err("relay authority is retired".into());
  }
  let bytes = std::fs::read(root.join("authority.json"))
    .map_err(|e| format!("relay authority unavailable: {e}"))?;
  let marker: Value =
    serde_json::from_slice(&bytes).map_err(|e| format!("invalid relay authority: {e}"))?;
  if config
    .relay
    .as_deref()
    .is_none_or(|relay| marker.get("machine").and_then(Value::as_str) != Some(relay))
    || marker.get("generation").and_then(Value::as_str) != config.generation.as_deref()
  {
    return Err("relay authority is not active".into());
  }
  Ok(())
}

/// Return saved authority metadata, including a retirement record if present.
pub(super) fn configuration(root: &Path) -> Result<Value, String> {
  let config = config::load(root)?;
  let retired = receipt(root, config.generation.as_deref())?
    .or(retirement(root)?.filter(|record| record.generation == config.generation));
  Ok(json!({"config":config, "retirement":retired}))
}

/// Change the active authority's configuration only from an exact snapshot.
pub(super) fn configure(
  root: &Path,
  store: &Store,
  expected: &config::Config,
  next: config::Config,
) -> Result<Value, String> {
  config::validate(&next)?;
  let saved = store.configure(|tasks| {
    require(root)?;
    config::update(root, |current| {
      if current != expected {
        return Err("configuration conflict".into());
      }
      if current.generation != next.generation || current.relay != next.relay {
        return Err("authority identity cannot change through config".into());
      }
      validate_changes(current, &next, tasks)?;
      *current = next;
      Ok(())
    })
  })?;
  super::encode(saved)
}

/// Enroll an unused or retired endpoint with a fresh authority generation.
pub(super) fn enroll(
  root: &Path,
  store: &Store,
  expected: &config::Config,
  next: config::Config,
) -> Result<Value, String> {
  config::validate(&next)?;
  let (relay, generation) = identity(&next)?;
  let saved = store.configure(|tasks| {
    let retired = retirement(root)?;
    if retired_generation(root, Some(&generation))? {
      return Err("retired generation cannot be enrolled".into());
    }
    let saved = config::update(root, |current| {
      if *current == next {
        check_existing_marker(root)?;
        return Ok(());
      }
      if current != expected {
        return Err("configuration conflict".into());
      }
      if root.join("authority.json").exists() || (retired.is_none() && current.relay.is_some()) {
        return Err("relay authority already exists".into());
      }
      if tasks.iter().any(|task| !terminal(task)) {
        return Err("cannot enroll with unfinished tasks".into());
      }
      *current = next;
      Ok(())
    })?;
    durable::write(
      &root.join("authority.json"),
      &serde_json::to_vec(&json!({"machine":relay,"generation":generation}))
        .map_err(|e| e.to_string())?,
    )?;
    Ok(saved)
  })?;
  super::encode(saved)
}

fn check_existing_marker(root: &Path) -> Result<(), String> {
  if root.join("authority.json").exists() {
    require(root)?;
  }
  Ok(())
}

fn identity(config: &config::Config) -> Result<(String, String), String> {
  let relay = config
    .relay
    .as_deref()
    .filter(|name| !name.is_empty())
    .ok_or("relay is not configured")?;
  let generation = config
    .generation
    .as_deref()
    .filter(|value| !value.is_empty())
    .ok_or("authority generation is required")?;
  if !super::super::store::safe(generation) || generation == "legacy" {
    return Err("invalid authority generation".into());
  }
  Ok((relay.to_owned(), generation.to_owned()))
}

fn validate_changes(
  current: &config::Config,
  next: &config::Config,
  tasks: &[super::super::store::Task],
) -> Result<(), String> {
  for task in tasks.iter().filter(|task| occupied(task)) {
    let name = task
      .machine
      .as_deref()
      .ok_or("occupied task has no machine")?;
    let same = current
      .machines
      .get(name)
      .zip(next.machines.get(name))
      .is_some_and(|(old, new)| {
        old.root == new.root && old.agents == new.agents && old.capacity == new.capacity
      });
    if !same {
      return Err(format!("cannot change occupied machine {name}"));
    }
  }
  Ok(())
}

fn terminal(task: &super::super::store::Task) -> bool {
  matches!(task.phase.as_str(), "done" | "cancelled" | "failed")
}

/// Persist retirement before clearing the authority marker.
pub(super) fn retire(
  root: &Path,
  store: &Store,
  expected: &config::Config,
  successor: &str,
) -> Result<Value, String> {
  if successor.is_empty() {
    return Err("successor is required".into());
  }
  let saved = store.configure(|tasks| {
    let record = Retirement {
      generation: expected.generation.clone(),
      successor: successor.into(),
    };
    let current = config::load(root)?;
    if current.generation != expected.generation
      && receipt(root, expected.generation.as_deref())?.as_ref() == Some(&record)
    {
      return Ok(current);
    }
    if tasks.iter().any(|task| !terminal(task)) {
      return Err("cannot retire relay with unfinished tasks".into());
    }
    let saved = config::update(root, |current| {
      let replay = receipt(root, expected.generation.as_deref())?.as_ref() == Some(&record)
        || retirement(root)?.as_ref() == Some(&record);
      if current != expected
        && !(replay && current.relay.is_none() && current.generation == expected.generation)
      {
        return Err("configuration conflict".into());
      }
      if !replay {
        require(root)?;
      }
      save_retirement(root, &record)?;
      current.relay = None;
      Ok(())
    })?;
    match std::fs::remove_file(root.join("authority.json")) {
      Ok(()) => std::fs::File::open(root)
        .and_then(|file| file.sync_all())
        .map_err(|e| e.to_string())?,
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
      Err(error) => return Err(error.to_string()),
    }
    Ok(saved)
  })?;
  super::encode(saved)
}

fn save_retirement(root: &Path, record: &Retirement) -> Result<(), String> {
  let path = receipt_path(root, record.generation.as_deref())?;
  let bytes = serde_json::to_vec(record).map_err(|e| e.to_string())?;
  durable::ensure_dir(path.parent().ok_or("missing retirement directory")?)?;
  match std::fs::read(&path) {
    Ok(saved) if saved == bytes => {
      std::fs::File::open(&path)
        .and_then(|file| file.sync_all())
        .map_err(|e| e.to_string())?;
      std::fs::File::open(path.parent().ok_or("missing retirement directory")?)
        .and_then(|dir| dir.sync_all())
        .map_err(|e| e.to_string())?;
    }
    Ok(_) => return Err("retirement receipt conflicts".into()),
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => durable::create(&path, &bytes)?,
    Err(error) => return Err(error.to_string()),
  }
  durable::write(&root.join("retired.json"), &bytes)
}

#[cfg(test)]
#[path = "tests/authority_test.rs"]
mod tests;
