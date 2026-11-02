//! Explicit authority enrollment and drained migration, fenced by generations.

use super::{Env, Value, client, config, json, service};
use toolu_mux::relay::store::Task;

mod pending;

struct Remote {
  config: config::Config,
  retirement: Option<Value>,
}

/// Select an authority without allowing stale local state to replace its registry.
pub(super) fn select(env: &Env, machine: &str) -> Result<Value, String> {
  toolu_mux::hc::remote::Target::new(Some(machine), Some("/")).map_err(|e| e.message)?;
  with_lock(env, || selected(env, machine))
}

/// Serialize local authority selection and registration changes.
pub(super) fn with_lock<T>(
  env: &Env,
  action: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
  use std::os::unix::fs::DirBuilderExt as _;
  let root = config::root(env);
  std::fs::DirBuilder::new()
    .recursive(true)
    .mode(0o700)
    .create(&root)
    .map_err(|e| e.to_string())?;
  toolu_state::lock::with_lock_strict(
    &root.join("selection"),
    toolu_state::lock::LockOptions {
      timeout: std::time::Duration::from_secs(30),
      stale: std::time::Duration::MAX,
    },
    action,
  )?
}

fn selected(env: &Env, machine: &str) -> Result<Value, String> {
  if pending::resume(env, machine)? {
    return service::start(env);
  }
  let local = config::load(&config::root(env))?;
  match local.relay.as_deref() {
    Some(_) => existing(env, machine, &local)?,
    None => initial(env, machine, &local)?,
  }
  service::start(env)
}

fn remote(env: &Env, machine: &str) -> Result<Remote, String> {
  let snapshot = client::configuration(&target(env, machine))?;
  Ok(Remote {
    config: serde_json::from_value(
      snapshot
        .get("config")
        .cloned()
        .ok_or("missing relay configuration")?,
    )
    .map_err(|e| e.to_string())?,
    retirement: snapshot.get("retirement").filter(|v| !v.is_null()).cloned(),
  })
}

fn target(env: &Env, machine: &str) -> Env {
  env.clone().with("TOOLU_RELAY_TARGET", machine)
}

fn save(env: &Env, next: &config::Config) -> Result<(), String> {
  config::update(&config::root(env), |cfg| {
    *cfg = next.clone();
    Ok(())
  })
  .map(|_cfg| ())
}

fn initial(env: &Env, machine: &str, local: &config::Config) -> Result<(), String> {
  let found = remote(env, machine)?;
  if let Some(retired) = found.retirement {
    return Err(format!(
      "relay retired; refresh the current authority {} first",
      retired
        .get("successor")
        .and_then(Value::as_str)
        .unwrap_or("named by relay status")
    ));
  }
  let mut next = if local.generation.is_some() {
    found.config.clone()
  } else {
    merged(&found.config, local)
  };
  if found.config.relay.is_some() {
    client::publish(&target(env, machine), &found.config, &next)?;
    return save(env, &next);
  }
  next.relay = Some(machine.into());
  next.generation = Some(toolu_state::lock::token());
  pending::begin(env, None, machine, &found.config, next)
}

fn existing(env: &Env, machine: &str, local: &config::Config) -> Result<(), String> {
  let current = local.relay.as_deref().ok_or("missing configured relay")?;
  let found = remote(env, current)?;
  if let Some(retirement) = &found.retirement {
    return resume_migration(env, machine, current, &found.config, retirement);
  }
  if current == machine && found.config.relay.is_none() {
    let mut pending = local.clone();
    pending
      .generation
      .get_or_insert_with(toolu_state::lock::token);
    return pending::begin(env, None, machine, &found.config, pending);
  }
  if current == machine {
    client::enroll(&target(env, machine), &found.config, &found.config)?;
    return save(env, &found.config);
  }
  let tasks: Vec<Task> =
    serde_json::from_value(client::call(&target(env, current), &json!({"op":"list"}))?)
      .map_err(|e| e.to_string())?;
  if tasks
    .iter()
    .any(|t| !matches!(t.phase.as_str(), "done" | "failed" | "cancelled"))
  {
    return Err("drain and settle all work before moving the relay".into());
  }
  let destination = remote(env, machine)?;
  if destination.config.relay.is_some() && destination.retirement.is_none() {
    return Err("destination is already an active authority".into());
  }
  enroll(
    env,
    machine,
    Some(current),
    &found.config,
    &destination.config,
  )
}

fn resume_migration(
  env: &Env,
  machine: &str,
  source_machine: &str,
  source: &config::Config,
  retirement: &Value,
) -> Result<(), String> {
  let successor = retirement
    .get("successor")
    .and_then(Value::as_str)
    .ok_or("invalid retirement receipt")?;
  if machine != successor {
    return Err(format!(
      "configured relay retired to {successor}; refresh that authority first"
    ));
  }
  let destination = remote(env, machine)?;
  if destination.retirement.is_none() && destination.config.relay.as_deref() == Some(machine) {
    return save(env, &destination.config);
  }
  enroll(
    env,
    machine,
    Some(source_machine),
    source,
    &destination.config,
  )
}

fn enroll(
  env: &Env,
  machine: &str,
  source_machine: Option<&str>,
  source: &config::Config,
  expected: &config::Config,
) -> Result<(), String> {
  let mut next = source.clone();
  next.relay = Some(machine.into());
  next.generation = Some(toolu_state::lock::token());
  pending::begin(
    env,
    source_machine.map(|name| (name, source)),
    machine,
    expected,
    next,
  )
}

fn merged(authoritative: &config::Config, requested: &config::Config) -> config::Config {
  let mut next = authoritative.clone();
  next.machines.extend(requested.machines.clone());
  next.recipes.extend(requested.recipes.clone());
  next
}

#[cfg(test)]
#[path = "tests/selection_test.rs"]
mod tests;
