//! Local registry cleanup, through the state loop or under engine ownership.

use std::collections::BTreeSet;
use std::sync::mpsc::Sender;

use clap::ArgMatches;
use serde_json::{Value, json};
use toolu_runtime::cli::Outcome;
use toolu_runtime::env::Env;

use crate::cleanup_state::{inspect, prune, safety};
use crate::lock::Held;
use crate::paths::Paths;
use crate::server::Engine;
use crate::verbs::{failed, json_out};

/// Handle cleanup on the state thread without pumping effects.
pub(crate) fn reply(
  engine: &mut Engine,
  request: &Value,
  reply: &Sender<Value>,
) -> Result<bool, String> {
  let dry_run = request
    .get("dry_run")
    .and_then(Value::as_bool)
    .unwrap_or(false);
  let paths = engine.paths.clone();
  let doc = apply(&paths, Some(engine), dry_run)?;
  let _sent = reply.send(doc);
  Ok(false)
}

/// Clean up without starting an engine or writing alongside a live owner.
pub(crate) fn run(matches: &ArgMatches, env: &Env) -> Outcome {
  let paths = Paths::from_env(env);
  let dry_run = matches.get_flag("dry-run");
  let result = match Held::acquire(&paths.lock(), "engine-busy") {
    Ok(_held) => apply(&paths, None, dry_run),
    Err(err) if err == "engine-busy" => crate::client::exchange_retry(
      &paths,
      crate::PROTOCOL,
      &json!({"op": "cleanup", "dry_run": dry_run}),
    ),
    Err(err) => Err(err),
  };
  match result {
    Ok(doc) => match doc.get("error").and_then(Value::as_str) {
      Some(err) => failed("toolu epic cleanup", err),
      None => json_out(&doc),
    },
    Err(err) => failed("toolu epic cleanup", &err),
  }
}

/// Persist the selection before pruning live state. The caller owns the engine.
pub(crate) fn apply(
  paths: &Paths,
  engine: Option<&mut Engine>,
  dry_run: bool,
) -> Result<Value, String> {
  crate::registry::update(paths, || {
    let mut registry = crate::registry::read(paths)?;
    let rows = registry
      .get("epics")
      .and_then(Value::as_array)
      .cloned()
      .unwrap_or_default();
    let mut recorded = Vec::new();
    crate::journal::each(&paths.journal_dir(), "", |record| {
      if record.name.ends_with("-intent")
        || record.name.ends_with("-done")
        || record.name == "epic-closed"
      {
        recorded.push(record);
      }
      Ok(())
    })?;
    let mut removed = Vec::new();
    let mut retained = Vec::new();
    let mut kept = Vec::new();
    for row in &rows {
      let (eligible, reason) = selection(paths, row, &rows, &recorded, engine.as_deref());
      let entry = json!({"entry": row, "reason": reason});
      if eligible {
        removed.push(entry);
      } else {
        retained.push(entry);
        kept.push(row.clone());
      }
    }
    if !dry_run && !removed.is_empty() {
      let epics = registry
        .get_mut("epics")
        .ok_or_else(|| "invalid registry".to_owned())?;
      *epics = json!(kept);
      let keys = removed
        .iter()
        .filter_map(|item| item.pointer("/entry/key").and_then(Value::as_str));
      commit(paths, &registry, engine, &keys.map(str::to_owned).collect())?;
    }
    Ok(json!({"dry_run": dry_run, "removed": removed, "retained": retained}))
  })
}

fn commit(
  paths: &Paths,
  registry: &Value,
  engine: Option<&mut Engine>,
  keys: &BTreeSet<String>,
) -> Result<(), String> {
  crate::disk::write_value(&paths.registry(), registry)?;
  if let Some(engine) = engine {
    prune(engine, keys);
    let record = crate::journal::Record::new(
      "action",
      "registry-cleanup",
      "",
      "",
      &format!("removed={}", keys.len()),
    );
    let stored = crate::journal::append(
      &paths.journal_dir(),
      &paths.journal_lock(),
      &record,
      engine.now,
    )
    .map_err(|err| format!("registry cleaned, but dashboard event could not be recorded: {err}"))?;
    engine.world.seq = engine.world.seq.max(stored.seq);
  }
  Ok(())
}

fn selection(
  paths: &Paths,
  row: &Value,
  rows: &[Value],
  recorded: &[crate::journal::Record],
  engine: Option<&Engine>,
) -> (bool, String) {
  let Some(key) = row
    .get("key")
    .and_then(Value::as_str)
    .filter(|key| !key.is_empty())
  else {
    return (false, "invalid registry entry".to_owned());
  };
  let Some(dir) = row
    .get("state_dir")
    .and_then(Value::as_str)
    .filter(|dir| !dir.is_empty())
  else {
    return (false, "invalid registry entry".to_owned());
  };
  if !std::path::Path::new(dir).is_absolute() {
    return (false, "relative state directory is ambiguous".to_owned());
  }
  if rows
    .iter()
    .filter(|row| row.get("key").and_then(Value::as_str) == Some(key))
    .count()
    != 1
  {
    return (false, "duplicate epic key".to_owned());
  }
  if let Some(engine) = engine {
    let keys = BTreeSet::from([key.to_owned()]);
    if let Some(reason) =
      safety(&engine.world, key).or_else(|| crate::cleanup_state::running(engine, &keys))
    {
      return (false, reason.to_owned());
    }
  }
  match inspect(paths, key, dir, recorded) {
    Ok(reason) => (true, reason.to_owned()),
    Err(reason) => (false, reason),
  }
}

#[cfg(test)]
#[path = "tests/cleanup_test.rs"]
mod tests;
