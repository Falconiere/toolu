//! `toolu epic status --fleet`: this machine's overview from the engine, and
//! each peer's fetched by this process, so the engine never waits on a peer.

use std::collections::BTreeMap;

use serde_json::{Value, json};
use toolu_runtime::cli::Outcome;
use toolu_runtime::config::{epic, load, secrets};
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;

use crate::PROTOCOL;
use crate::client::exchange_retry;
use crate::control::{ensure, registry_nonempty};
use crate::fleet;
use crate::model::World;
use crate::paths::Paths;
use crate::verbs::{failed, json_out};

/// Print the fleet document; peer states are data, so any state exits 0.
pub(crate) fn run(env: &Env) -> Outcome {
  match document(env) {
    Ok(value) => json_out(&value),
    Err(err) => failed("toolu epic status --fleet", &err),
  }
}

fn document(env: &Env) -> Result<Value, String> {
  let roots = Roots::new(env.clone(), None);
  let settings = epic::parse(&load::load(&roots, None)).map_err(|err| err.to_string())?;
  let secrets = secrets::load(&roots).map_err(|err| err.to_string())?;
  let local = local_overview(env)?;
  let targets = fleet::targets(&settings.peers, &secrets);
  let fetched = fleet::fetch_all(&fleet::client()?, &targets);
  let now = fleet::now_ms();
  let mut machines = vec![fleet::local_machine(local, now)];
  machines.extend(fleet::peer_machines(
    &targets,
    fetched,
    &mut BTreeMap::new(),
    now,
  ));
  Ok(fleet::document(&machines, now, &secrets))
}

/// The engine's overview, or the engine-down document with no registered epic.
fn local_overview(env: &Env) -> Result<Value, String> {
  ensure(env)?;
  let paths = Paths::from_env(env);
  if !registry_nonempty(&paths) {
    return Ok(crate::status::document(&World::new(0), false, None));
  }
  let value = exchange_retry(&paths, PROTOCOL, &json!({"op": "status"}))?;
  match value.get("error").and_then(Value::as_str) {
    Some(err) => Err(err.to_owned()),
    None => Ok(value),
  }
}

#[cfg(test)]
#[path = "tests/fleet_cli_test.rs"]
mod tests;
