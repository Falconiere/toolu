//! Status, wait, report and job verbs.

use std::path::Path;
use std::time::SystemTime;

use clap::ArgMatches;
use serde_json::{Value, json};
use toolu_protocol::host::Host;
use toolu_runtime::atomic::write_atomic;
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::invocation::{current_dir, current_exe};
use toolu_state::time::epoch_millis;

use crate::PROTOCOL;
use crate::client::exchange_retry;
use crate::control::{ensure, registry_nonempty};
use crate::disk::read_value;
use crate::job::run_job;
use crate::lock::live;
use crate::model::World;
use crate::paths::Paths;
use crate::status::document;
use crate::verbs::{env_of, failed, json_out, text};

pub(crate) fn status(ctx: &Ctx, matches: &ArgMatches) -> Outcome {
  let env = env_of(ctx);
  if let Err(err) = ensure(&env) {
    return failed("toolu epic status", &err);
  }
  let paths = Paths::from_env(&env);
  let epic = text(matches, "epic");
  if !registry_nonempty(&paths) {
    return json_out(&document(&World::new(0), false, epic));
  }
  let request = match epic {
    Some(epic) => json!({"op": "status", "epic": epic}),
    None => json!({"op": "status"}),
  };
  match exchange_retry(&paths, PROTOCOL, &request) {
    Ok(doc) => json_out(&doc),
    Err(err) => failed("toolu epic status", &err),
  }
}

pub(crate) fn pause(ctx: &Ctx, matches: &ArgMatches, pausing: bool) -> Outcome {
  let op = if pausing { "pause" } else { "resume" };
  let request = match text(matches, "epic") {
    Some(epic) => json!({"op": op, "epic": epic}),
    None => json!({"op": op}),
  };
  ask(ctx, op, &request)
}

pub(crate) fn ack(ctx: &Ctx, matches: &ArgMatches) -> Outcome {
  let key = text(matches, "key").unwrap_or("");
  let request = json!({"op": "ack", "key": key});
  ask(ctx, "ack", &request)
}

pub(crate) fn answer(ctx: &Ctx, matches: &ArgMatches) -> Outcome {
  let key = text(matches, "key").unwrap_or("");
  let body = text(matches, "text").unwrap_or("");
  let request = json!({"op": "answer", "key": key, "text": body});
  ask(ctx, "answer", &request)
}

pub(crate) fn wait(ctx: &Ctx, matches: &ArgMatches) -> Outcome {
  let max = matches
    .get_one::<u64>("max-seconds")
    .copied()
    .unwrap_or_else(|| wait_default(ctx.host));
  let env = env_of(ctx);
  if let Err(err) = ensure(&env) {
    return failed("toolu epic wait", &err);
  }
  let paths = Paths::from_env(&env);
  if !live(&paths.lock()) {
    return json_out(&json!({"state": "waiting"}));
  }
  let request = json!({"op": "wait", "max_seconds": max});
  ask(ctx, "wait", &request)
}

pub(crate) fn report(ctx: &Ctx, matches: &ArgMatches) -> Outcome {
  let Some(phase) = text(matches, "phase") else {
    return failed("toolu epic report", "phase is required");
  };
  let Some(file) = text(matches, "status-file") else {
    return failed("toolu epic report", "status-file is required");
  };
  let (key, state_dir) = match located(Path::new(file)) {
    Ok(pair) => pair,
    Err(err) => return failed("toolu epic report", &err),
  };
  let token = format!("r{}-{key}", epoch_millis(SystemTime::now()));
  let env = env_of(ctx);
  if let Err(err) = ensure(&env) {
    return failed("toolu epic report", &err);
  }
  let paths = Paths::from_env(&env);
  let request = json!({
    "op": "report",
    "token": token,
    "key": key,
    "epic": epic_for(&paths, &state_dir),
    "state_dir": state_dir,
    "phase": phase,
    "pr": matches.get_one::<u64>("pr").copied(),
    "note": text(matches, "note").unwrap_or(""),
  });
  ask(ctx, "report", &request)
}

pub(crate) fn ask(ctx: &Ctx, verb: &str, request: &Value) -> Outcome {
  let env = env_of(ctx);
  if let Err(err) = ensure(&env) {
    return failed(&format!("toolu epic {verb}"), &err);
  }
  match exchange_retry(&Paths::from_env(&env), PROTOCOL, request) {
    Ok(doc) => json_out(&doc),
    Err(err) => failed(&format!("toolu epic {verb}"), &err),
  }
}

pub(crate) fn job(matches: &ArgMatches) -> Outcome {
  let cwd = match current_dir() {
    Ok(cwd) => cwd,
    Err(err) => {
      let err = err.to_string();
      return failed("toolu epic job", &err);
    }
  };
  let argv: Vec<String> = matches
    .get_many::<String>("argv")
    .map(|values| values.cloned().collect())
    .unwrap_or_default();
  if argv.is_empty() {
    return failed("toolu epic job", "a command is required");
  }
  run_job(&argv, &cwd)
}

pub(crate) fn service(ctx: &Ctx) -> Outcome {
  let env = env_of(ctx);
  let paths = Paths::from_env(&env);
  let Some(exe) = current_exe() else {
    return failed("toolu epic service", "cannot find the current executable");
  };
  match install_unit(&paths, &env.home(), &exe) {
    Ok(()) => Outcome::data(format!("toolu epic: unit {}", paths.service().display())),
    Err(err) => failed("toolu epic service", &err),
  }
}

pub(crate) fn install_unit(paths: &Paths, home: &Path, exe: &Path) -> Result<(), String> {
  let body = format!(
    "[Unit]\nDescription=toolu epic engine\n\n[Service]\nExecStart={} epic engine\nEnvironment=TOOLU_RESOURCE_HOME={}\n\n[Install]\nWantedBy=default.target\n",
    exe.display(),
    paths.root.display()
  );
  let user = home.join(".config/systemd/user/toolu-epic.service");
  if !write_atomic(&paths.service(), &body) {
    return Err(format!("could not write {}", paths.service().display()));
  }
  if !write_atomic(&user, &body) {
    return Err(format!("could not write {}", user.display()));
  }
  Ok(())
}

pub(crate) fn located(path: &Path) -> Result<(String, String), String> {
  let key = path
    .file_stem()
    .and_then(|name| name.to_str())
    .ok_or("status file needs a key")?;
  let state = path
    .parent()
    .and_then(Path::parent)
    .ok_or("status file is not under status/")?;
  Ok((key.to_owned(), state.display().to_string()))
}

pub(crate) fn epic_for(paths: &Paths, state_dir: &str) -> String {
  let Ok(value) = read_value(&paths.registry()) else {
    return String::new();
  };
  let Some(rows) = value.get("epics").and_then(Value::as_array) else {
    return String::new();
  };
  rows
    .iter()
    .find_map(|row| {
      (row.get("state_dir").and_then(Value::as_str) == Some(state_dir)).then(|| {
        row
          .get("key")
          .and_then(Value::as_str)
          .unwrap_or("")
          .to_owned()
      })
    })
    .unwrap_or_default()
}

pub(crate) fn wait_default(host: Option<Host>) -> u64 {
  match host {
    Some(Host::Codex | Host::Opencode | Host::Cursor | Host::Hermes) => 480,
    _ => 2700,
  }
}

#[cfg(test)]
#[path = "tests/query_test.rs"]
mod tests;
