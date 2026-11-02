//! Read-only worker connection and reservation status.

use super::{
  config::{self, Config, Machine},
  store::{Store, Task, occupied},
};
use crate::hc::Client;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use toolu_runtime::env::Env;

/// Return relay state and observed HC connections without probing agent readiness.
///
/// # Errors
/// The saved registry or task ledger cannot be read.
pub fn status(root: &Path, env: &Env) -> Result<Value, String> {
  let config = config::load(root)?;
  let (tasks, cooldowns) = Store::new(root).status_snapshot()?;
  let now = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map_err(|e| e.to_string())?
    .as_secs();
  let health = std::fs::read_to_string(root.join("health.json"))
    .ok()
    .and_then(|s| serde_json::from_str::<Value>(&s).ok());
  let connections = Client::new(env)
    .call(&["machine", "ls"], Duration::from_secs(10))
    .map_err(|e| format!("{}: {}", e.code, e.message))
    .and_then(|value| {
      value
        .get("machines")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| "invalid HC machine list".to_owned())
    });
  let workers = project(
    &config,
    &tasks,
    connections.as_deref().ok(),
    &cooldowns,
    now,
  );
  Ok(
    json!({"config":config, "tasks":tasks, "health":health, "workers":workers, "connection_error":connections.err()}),
  )
}

fn project(
  config: &Config,
  tasks: &[Task],
  connections: Option<&[Value]>,
  cooldowns: &BTreeMap<String, u64>,
  now: u64,
) -> Vec<Value> {
  config
    .machines
    .values()
    .map(|machine| {
      worker(
        machine,
        tasks,
        connections,
        cooldowns.get(&machine.name).copied(),
        now,
      )
    })
    .collect()
}

fn worker(
  machine: &Machine,
  tasks: &[Task],
  connections: Option<&[Value]>,
  cooldown: Option<u64>,
  now: u64,
) -> Value {
  let count = tasks
    .iter()
    .filter(|task| task.machine.as_deref() == Some(&machine.name) && occupied(task))
    .count();
  let count = u32::try_from(count).unwrap_or(u32::MAX);
  let available = if machine.draining || cooldown.is_some_and(|until| until > now) {
    0
  } else {
    machine.capacity.saturating_sub(count)
  };
  let errors: Vec<_> = tasks
    .iter()
    .filter(|task| {
      task.phase == "queued"
        && task.failures > 0
        && task
          .worker
          .as_ref()
          .is_some_and(|saved| saved.name == machine.name)
        && task.note.is_some()
    })
    .map(|task| json!({"key":task.request.key,"note":task.note,"next_retry":task.next_at}))
    .collect();
  json!({"name":machine.name, "connection":connection(machine, connections), "draining":machine.draining, "capacity":machine.capacity, "occupied":count, "available":available, "agents":machine.agents, "preparation_errors":errors, "preparation_cooldown_until":cooldown})
}

fn connection(machine: &Machine, connections: Option<&[Value]>) -> &'static str {
  let Some(connections) = connections else {
    return "unknown";
  };
  let Some(found) = connections
    .iter()
    .find(|item| item.get("name").and_then(Value::as_str) == Some(&machine.name))
  else {
    return "unconfigured";
  };
  match found.get("state").and_then(Value::as_str) {
    Some("connected") => "connected",
    Some("disconnected") => "disconnected",
    _ => "unknown",
  }
}

#[cfg(test)]
#[path = "tests/status_test.rs"]
mod tests;
