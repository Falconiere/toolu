//! Foreground engine, ensure, start and the service unit.

use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use clap::ArgMatches;
use serde_json::{Value, json};
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Outcome;
use toolu_runtime::env::Env;
use toolu_runtime::invocation::current_exe;
use toolu_runtime::process::{Spec, detach};

use crate::PROTOCOL;
use crate::client::exchange_retry;
use crate::disk::{read_value, write_value};
use crate::lock::live;
use crate::paths::Paths;
use crate::server::Fault;
use crate::socket::serve;
use crate::verbs::{failed, text};

pub(crate) fn engine(matches: &ArgMatches, env: &Env) -> Outcome {
  if matches.get_flag("replace") {
    let paths = Paths::from_env(env);
    if paths.socket().exists() {
      let _replaced = exchange_retry(&paths, PROTOCOL, &json!({"op": "replace"}));
      wait_until_free(&paths);
    }
  }
  if matches.get_flag("ensure") {
    return match ensure(env) {
      Ok(()) => Outcome::data(String::new()),
      Err(err) => failed("toolu epic engine", &err),
    };
  }
  foreground(env)
}

pub(crate) fn foreground(env: &Env) -> Outcome {
  let paths = Paths::from_env(env);
  let scripts = env.get("TOOLU_EPIC_SCRIPTS").map(PathBuf::from);
  match serve(paths, scripts, fault_of(env), PROTOCOL) {
    Ok(()) => Outcome::data(String::new()),
    Err(err) if err == "fault" => {
      Outcome::failed(Exit::TempFail, "toolu epic engine: fault".to_owned())
    }
    Err(err) => failed("toolu epic engine", &err),
  }
}

pub(crate) fn fault_of(env: &Env) -> Fault {
  match env.get("TOOLU_EPIC_FAULT") {
    Some("before-merge") => Fault::BeforeMerge,
    Some("after-merge") => Fault::AfterMerge,
    Some("before-merge-record") => Fault::BeforeMergeRecord,
    _ => Fault::None,
  }
}

pub(crate) fn ensure(env: &Env) -> Result<(), String> {
  let paths = Paths::from_env(env);
  if !registry_nonempty(&paths) || live(&paths.lock()) {
    return Ok(());
  }
  detach_engine(env)
}

pub(crate) fn detach_engine(env: &Env) -> Result<(), String> {
  let exe = current_exe().ok_or_else(|| "cannot find the current executable".to_owned())?;
  let mut spec = Spec::new([
    exe.display().to_string(),
    "epic".to_owned(),
    "engine".to_owned(),
  ]);
  spec.env = Some(env.clone());
  detach(&spec).map(|_| ()).map_err(|err| format!("{err:?}"))
}

fn wait_until_free(paths: &Paths) {
  for _ in 0..50 {
    if !live(&paths.lock()) {
      return;
    }
    thread::sleep(Duration::from_millis(20));
  }
}

pub(crate) fn start(matches: &ArgMatches, env: &Env) -> Outcome {
  let Some(dir) = text(matches, "state-dir") else {
    return failed("toolu epic start", "state-dir is required");
  };
  let paths = Paths::from_env(env);
  if let Err(err) = register(&paths, dir) {
    return failed("toolu epic start", &err);
  }
  if live(&paths.lock()) {
    return Outcome::data(String::new());
  }
  match detach_engine(env) {
    Ok(()) => Outcome::data(String::new()),
    Err(err) => failed("toolu epic start", &err),
  }
}

pub(crate) fn register(paths: &Paths, state_dir: &str) -> Result<(), String> {
  let current = read_value(&paths.registry())?;
  let mut epics = current
    .get("epics")
    .and_then(Value::as_array)
    .cloned()
    .unwrap_or_default();
  let present = epics
    .iter()
    .any(|row| row.get("state_dir").and_then(Value::as_str) == Some(state_dir));
  if !present {
    epics.push(json!({"key": epic_key(Path::new(state_dir)), "state_dir": state_dir}));
  }
  write_value(&paths.registry(), &json!({"version": 1, "epics": epics}))
}

pub(crate) fn epic_key(state_dir: &Path) -> String {
  if let Ok(graph) = read_value(&state_dir.join("graph.json"))
    && let Some(key) = graph
      .get("epic")
      .and_then(Value::as_str)
      .filter(|key| !key.is_empty())
  {
    return key.to_owned();
  }
  state_dir
    .file_name()
    .and_then(|name| name.to_str())
    .map_or_else(|| state_dir.display().to_string(), str::to_owned)
}

pub(crate) fn registry_nonempty(paths: &Paths) -> bool {
  let Ok(value) = read_value(&paths.registry()) else {
    return false;
  };
  value
    .get("epics")
    .and_then(Value::as_array)
    .is_some_and(|rows| !rows.is_empty())
}

#[cfg(test)]
#[path = "tests/control_test.rs"]
mod tests;
