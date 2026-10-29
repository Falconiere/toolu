//! Status, wait, report and job verbs.

use std::path::Path;
use std::time::SystemTime;

use clap::{Arg, ArgAction, ArgMatches, Command};
use serde_json::{Value, json};
use toolu_protocol::host::Host;
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::env::Env;
use toolu_runtime::invocation::current_dir;

use crate::PROTOCOL;
use crate::client::exchange_retry;
use crate::control::{ensure, registry_nonempty};
use crate::disk::{read_value, write_status, write_value};
use crate::hc_report::report_token;
use crate::job::run_job;
use crate::lock::live;
use crate::model::World;
use crate::paths::Paths;
use crate::status::document;
use crate::verbs::{failed, json_out, text};

/// `toolu epic status [EPIC | --fleet]`.
pub(crate) fn status_command() -> Command {
  Command::new("status")
    .about("Print the engine, issues and attention")
    .arg(
      Arg::new("epic")
        .value_name("EPIC")
        .help("Epic key, or a trailing issue number"),
    )
    .arg(
      Arg::new("fleet")
        .long("fleet")
        .action(ArgAction::SetTrue)
        .conflicts_with("epic")
        .help("Print this machine and every configured peer engine"),
    )
}

/// Read one status document through the engine or report that its key is unknown.
pub(crate) fn status(matches: &ArgMatches, env: &Env) -> Outcome {
  if matches.get_flag("fleet") {
    return crate::fleet_cli::run(env);
  }
  if let Err(err) = ensure(env) {
    return failed("toolu epic status", &err);
  }
  let paths = Paths::from_env(env);
  let epic = text(matches, "epic");
  if !registry_nonempty(&paths) {
    if let Some(key) = epic {
      return failed(
        "toolu epic status",
        &format!("unknown epic or issue: {key}"),
      );
    }
    return json_out(&document(&World::new(0), false, epic));
  }
  let request = match epic {
    Some(epic) => json!({"op": "status", "epic": epic}),
    None => json!({"op": "status"}),
  };
  match exchange_retry(&paths, PROTOCOL, &request) {
    Ok(doc) => match doc.get("error").and_then(Value::as_str) {
      Some(err) => failed("toolu epic status", err),
      None => json_out(&doc),
    },
    Err(err) => failed("toolu epic status", &err),
  }
}

/// Print the merge queue through the engine; with no registered epic it is empty.
pub(crate) fn queue(matches: &ArgMatches, env: &Env) -> Outcome {
  let epic = text(matches, "epic");
  if !registry_nonempty(&Paths::from_env(env)) {
    return json_out(&crate::queue::document(&World::new(0), epic));
  }
  let request = match epic {
    Some(epic) => json!({"op": "queue", "epic": epic}),
    None => json!({"op": "queue"}),
  };
  ask(env, "queue", &request)
}

/// Set the pause state through the control socket.
pub(crate) fn pause(matches: &ArgMatches, env: &Env, pausing: bool) -> Outcome {
  let op = if pausing { "pause" } else { "resume" };
  let request = match text(matches, "epic") {
    Some(epic) => json!({"op": op, "epic": epic}),
    None => json!({"op": op}),
  };
  ask(env, op, &request)
}

/// Acknowledge the attention items of one key through the control socket.
pub(crate) fn ack(ctx: &Ctx, matches: &ArgMatches, env: &Env) -> Outcome {
  let key = text(matches, "key").unwrap_or("");
  let request = json!({"op": "ack", "key": key, "by": answerer(ctx, None)});
  ask(env, "ack", &request)
}

/// Prompt the worker with an answer through the control socket.
pub(crate) fn answer(ctx: &Ctx, matches: &ArgMatches, env: &Env) -> Outcome {
  let key = text(matches, "key").unwrap_or("");
  let body = text(matches, "text").unwrap_or("");
  let by = answerer(ctx, text(matches, "by"));
  let request = json!({"op": "answer", "key": key, "text": body, "by": by});
  ask_checked(env, "answer", &request)
}

/// `--by`, else the `--host` name, else `user`.
pub(crate) fn answerer(ctx: &Ctx, by: Option<&str>) -> String {
  by.filter(|by| !by.is_empty()).map_or_else(
    || ctx.host.map_or("user", Host::name).to_owned(),
    str::to_owned,
  )
}

/// Wait for a bounded engine judgment.
pub(crate) fn wait(ctx: &Ctx, matches: &ArgMatches, env: &Env) -> Outcome {
  let max = matches
    .get_one::<u64>("max-seconds")
    .copied()
    .unwrap_or_else(|| wait_default(ctx.host));
  if let Err(err) = ensure(env) {
    return failed("toolu epic wait", &err);
  }
  let paths = Paths::from_env(env);
  if !live(&paths.lock()) {
    return json_out(&json!({"state": "waiting"}));
  }
  let request = match text(matches, "epic") {
    Some(epic) => json!({"op": "wait", "max_seconds": max, "epic": epic}),
    None => json!({"op": "wait", "max_seconds": max}),
  };
  ask_checked(env, "wait", &request)
}

/// Persist and submit a worker phase report.
pub(crate) fn report(matches: &ArgMatches, env: &Env) -> Outcome {
  if let Some(outcome) = crate::hc_report::report(matches, env) {
    return outcome;
  }
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
  if let Err(err) = ensure(env) {
    return failed("toolu epic report", &err);
  }
  let paths = Paths::from_env(env);
  let pr = matches.get_one::<u64>("pr").copied();
  let note = text(matches, "note").unwrap_or("");
  let token = report_token(&key);
  let written = write_status(Path::new(file), phase, pr, note, SystemTime::now());
  if let Err(err) = written {
    return failed("toolu epic report", &err);
  }
  let request = json!({
    "op": "report",
    "token": token,
    "key": key,
    "epic": epic_for(&paths, &state_dir),
    "state_dir": state_dir,
    "phase": phase,
    "pr": pr,
    "note": note,
  });
  submit_spooled(&paths, "report", false, &token, &request)
}

/// Send a worker notice to the orchestrator, or one sibling issue.
pub(crate) fn notify(matches: &ArgMatches, env: &Env) -> Outcome {
  let Some(kind) = text(matches, "kind") else {
    return failed("toolu epic notify", "kind is required");
  };
  let Some(file) = text(matches, "status-file") else {
    return failed("toolu epic notify", "status-file is required");
  };
  let (key, state_dir) = match located(Path::new(file)) {
    Ok(pair) => pair,
    Err(err) => return failed("toolu epic notify", &err),
  };
  if let Err(err) = ensure(env) {
    return failed("toolu epic notify", &err);
  }
  let paths = Paths::from_env(env);
  let token = report_token(&key);
  let request = json!({
    "op": "notify",
    "token": token,
    "key": key,
    "epic": epic_for(&paths, &state_dir),
    "state_dir": state_dir,
    "kind": kind,
    "note": text(matches, "note").unwrap_or(""),
    "to": text(matches, "to").unwrap_or(""),
  });
  submit_spooled(&paths, "notify", true, &token, &request)
}

/// Spool `req` under `token`, then deliver it when the engine is live.
/// An unreachable engine keeps the spool and still answers `ok`. With
/// `checked`, an `error` reply (a refused request) is a failure.
fn submit_spooled(paths: &Paths, verb: &str, checked: bool, token: &str, req: &Value) -> Outcome {
  let path = match crate::client::spool_path(&paths.root, token) {
    Ok(path) => path,
    Err(err) => return failed(&format!("toolu epic {verb}"), &err),
  };
  if let Err(err) = write_value(&path, req) {
    return failed(&format!("toolu epic {verb}"), &err);
  }
  if !paths.socket().exists() {
    return json_out(&json!({"ok": true}));
  }
  match exchange_retry(paths, PROTOCOL, req) {
    Ok(doc) => match doc.get("error").and_then(Value::as_str) {
      Some(err) if checked => failed(&format!("toolu epic {verb}"), err),
      _ => json_out(&doc),
    },
    Err(err) if err.starts_with("connect:") => json_out(&json!({"ok": true})),
    Err(err) => failed(&format!("toolu epic {verb}"), &err),
  }
}

/// Exchange a control request and format its response.
pub(crate) fn ask(env: &Env, verb: &str, request: &Value) -> Outcome {
  if let Err(err) = ensure(env) {
    return failed(&format!("toolu epic {verb}"), &err);
  }
  match exchange_retry(&Paths::from_env(env), PROTOCOL, request) {
    Ok(doc) => json_out(&doc),
    Err(err) => failed(&format!("toolu epic {verb}"), &err),
  }
}

/// As [`ask`], but an `error` reply from the engine is a failure.
pub(crate) fn ask_checked(env: &Env, verb: &str, request: &Value) -> Outcome {
  if let Err(err) = ensure(env) {
    return failed(&format!("toolu epic {verb}"), &err);
  }
  match exchange_retry(&Paths::from_env(env), PROTOCOL, request) {
    Ok(doc) => match doc.get("error").and_then(Value::as_str) {
      Some(err) => failed(&format!("toolu epic {verb}"), err),
      None => json_out(&doc),
    },
    Err(err) => failed(&format!("toolu epic {verb}"), &err),
  }
}

/// Run one managed expensive job.
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

/// Resolve a status file to its issue key and state directory.
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

/// Find the registered epic for a state directory.
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

/// Return the host's default judgment wait in seconds.
pub(crate) fn wait_default(host: Option<Host>) -> u64 {
  match host {
    Some(Host::Codex | Host::Cursor | Host::Hermes) => 480,
    _ => 2700,
  }
}

#[cfg(test)]
#[path = "tests/query_test.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/query_notify_test.rs"]
mod notify_tests;

#[cfg(test)]
#[path = "tests/report_test.rs"]
mod report_tests;

#[cfg(test)]
#[path = "tests/query_queue_test.rs"]
mod queue_tests;
