//! Registry editing is serialized with the relay's capacity ledger.

use super::{ArgMatches, Env, Store, Value, client, config, json, occupied, service, value};
use toolu_mux::relay::config::Machine;

/// Apply an explicit registration change.
pub(super) fn setup_machines(matches: &ArgMatches, env: &Env) -> Result<Value, String> {
  match matches.subcommand() {
    None | Some(("list", _)) => {
      serde_json::to_value(config::load(&config::root(env))?).map_err(|e| e.to_string())
    }
    Some(("register", sub)) => {
      let machine = Machine {
        name: value(sub, "name").into(),
        root: value(sub, "root").into(),
        agents: sub
          .get_many::<String>("agent")
          .map(|v| v.cloned().collect())
          .unwrap_or_default(),
        capacity: sub.get_one::<u32>("capacity").copied().unwrap_or(1),
        draining: false,
      };
      change(env, |cfg, tasks| {
        let mut machine = machine;
        if let Some(saved) = cfg.machines.get(&machine.name) {
          machine.draining = saved.draining;
        }
        if tasks
          .iter()
          .any(|t| occupied(t) && t.machine.as_ref() == Some(&machine.name))
          && cfg.machines.get(&machine.name) != Some(&machine)
        {
          return Err("worker has active assignments; drain and wait first".into());
        }
        config::register(cfg, machine)
      })
    }
    Some(("remove", sub)) => change(env, |cfg, tasks| {
      let name = value(sub, "name");
      if tasks
        .iter()
        .any(|t| occupied(t) && t.machine.as_deref() == Some(name))
      {
        return Err("worker has active assignments; drain and wait first".into());
      }
      cfg.machines.remove(name).ok_or("unknown worker")?;
      Ok(())
    }),
    Some(("recipe", sub)) => recipe(sub, env),
    _ => Err("unknown machine setup operation".into()),
  }
}

fn recipe(matches: &ArgMatches, env: &Env) -> Result<Value, String> {
  let bytes = std::fs::read(value(matches, "file")).map_err(|e| e.to_string())?;
  let recipe: Vec<Vec<String>> = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
  change(env, |cfg, _tasks| {
    if recipe.is_empty() {
      cfg.recipes.remove(value(matches, "origin"));
    } else {
      cfg.recipes.insert(value(matches, "origin").into(), recipe);
    }
    Ok(())
  })
}

/// Inspect or change worker admission.
pub(super) fn machine_operation(matches: &ArgMatches, env: &Env) -> Result<Value, String> {
  match matches.subcommand() {
    None | Some(("status", _)) => service::status(env),
    Some((operation @ ("drain" | "resume"), sub)) => change(env, |cfg, _tasks| {
      cfg
        .machines
        .get_mut(value(sub, "name"))
        .ok_or("unknown worker")?
        .draining = operation == "drain";
      Ok(())
    }),
    _ => Err("unknown machine operation".into()),
  }
}

#[cfg(test)]
#[path = "tests/machines_test.rs"]
mod tests;

fn change(
  env: &Env,
  change: impl FnOnce(&mut config::Config, &[toolu_mux::relay::store::Task]) -> Result<(), String>,
) -> Result<Value, String> {
  super::selection::with_lock(env, || change_locked(env, change))
}

fn change_locked(
  env: &Env,
  change: impl FnOnce(&mut config::Config, &[toolu_mux::relay::store::Task]) -> Result<(), String>,
) -> Result<Value, String> {
  let root = config::root(env);
  let cfg = config::load(&root)?;
  if cfg.relay.is_some() {
    let snapshot = client::configuration(env)?;
    if !snapshot.get("retirement").is_none_or(Value::is_null) {
      return Err(
        "configured relay has retired; refresh it with toolu setup relay before editing machines"
          .into(),
      );
    }
    let expected: config::Config = serde_json::from_value(
      snapshot
        .get("config")
        .cloned()
        .ok_or("missing relay configuration")?,
    )
    .map_err(|e| e.to_string())?;
    let tasks: Vec<toolu_mux::relay::store::Task> =
      serde_json::from_value(client::call(env, &json!({"op":"list"}))?)
        .map_err(|e| e.to_string())?;
    let mut next = expected.clone();
    change(&mut next, &tasks)?;
    config::validate(&next)?;
    client::publish(env, &expected, &next)?;
    config::update(&root, |cfg| {
      *cfg = next;
      Ok(())
    })?;
  } else {
    Store::new(&root).configure(|tasks| config::update(&root, |cfg| change(cfg, tasks)))?;
  }
  serde_json::to_value(config::load(&root)?).map_err(|e| e.to_string())
}
