//! Durable authority selection outbox, written before retiring or enrolling.

use super::{Env, client, config, json, remote, save, target};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Source {
  machine: String,
  expected: config::Config,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pending {
  source: Option<Source>,
  destination: String,
  expected_destination: config::Config,
  next: config::Config,
}

fn path(env: &Env) -> PathBuf {
  config::root(env).join("pending-relay.json")
}

/// Save the exact generation and both endpoints before any authority mutation.
pub(super) fn begin(
  env: &Env,
  source: Option<(&str, &config::Config)>,
  destination: &str,
  expected: &config::Config,
  next: config::Config,
) -> Result<(), String> {
  let pending = Pending {
    source: source.map(|(machine, expected)| Source {
      machine: machine.into(),
      expected: expected.clone(),
    }),
    destination: destination.into(),
    expected_destination: expected.clone(),
    next,
  };
  write(&path(env), &pending)?;
  apply(env, pending)
}

/// Resume a saved selection without mistaking a returning endpoint's old history for its source.
pub(super) fn resume(env: &Env, requested: &str) -> Result<bool, String> {
  let bytes = match std::fs::read(path(env)) {
    Ok(bytes) => bytes,
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
    Err(error) => return Err(error.to_string()),
  };
  let pending: Pending =
    serde_json::from_slice(&bytes).map_err(|e| format!("invalid pending selection: {e}"))?;
  if pending.destination != requested {
    return Err(format!(
      "complete pending relay selection for {} first",
      pending.destination
    ));
  }
  apply(env, pending)?;
  Ok(true)
}

fn write(path: &Path, pending: &Pending) -> Result<(), String> {
  toolu_mux::relay::write_state(
    path,
    &serde_json::to_vec(pending).map_err(|e| e.to_string())?,
  )
}

fn apply(env: &Env, mut pending: Pending) -> Result<(), String> {
  let found = remote(env, &pending.destination)?;
  if found.config.generation == pending.next.generation {
    if let Some(retirement) = &found.retirement {
      let mut retired = found.config.clone();
      retired.relay = Some(pending.destination.clone());
      finish(env, &retired)?;
      return Err(format!(
        "accepted relay selection has retired; refresh its successor {}",
        retirement
          .get("successor")
          .and_then(serde_json::Value::as_str)
          .unwrap_or("from relay status")
      ));
    }
    if found.config.relay == pending.next.relay
      && client::call(
        &target(env, &pending.destination),
        &json!({"op":"authority"}),
      )
      .is_ok()
    {
      return finish(env, &found.config);
    }
  }
  retire_source(env, &mut pending)?;
  client::enroll(
    &target(env, &pending.destination),
    &pending.expected_destination,
    &pending.next,
  )?;
  client::call(
    &target(env, &pending.destination),
    &json!({"op":"authority"}),
  )?;
  let accepted = remote(env, &pending.destination)?;
  finish(env, &accepted.config)
}

fn retire_source(env: &Env, pending: &mut Pending) -> Result<(), String> {
  let Some(source) = &mut pending.source else {
    return Ok(());
  };
  let current = remote(env, &source.machine)?;
  if current.retirement.is_none() && current.config.generation == source.expected.generation {
    source.expected = current.config;
    pending.next.machines.clone_from(&source.expected.machines);
    pending.next.recipes.clone_from(&source.expected.recipes);
    write(&path(env), pending)?;
  }
  let source = pending.source.as_ref().ok_or("missing migration source")?;
  client::call(
    &target(env, &source.machine),
    &json!({"op":"retire", "expected":source.expected, "successor":pending.destination}),
  )?;
  Ok(())
}

fn finish(env: &Env, accepted: &config::Config) -> Result<(), String> {
  save(env, accepted)?;
  let path = path(env);
  std::fs::remove_file(&path).map_err(|e| e.to_string())?;
  std::fs::File::open(path.parent().ok_or("missing selection directory")?)
    .and_then(|dir| dir.sync_all())
    .map_err(|e| e.to_string())
}

#[cfg(test)]
#[path = "tests/pending_test.rs"]
mod tests;
