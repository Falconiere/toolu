//! `toolu epic finish` and `toolu epic release`.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use clap::{Arg, ArgMatches, Command};
use serde_json::{Value, json};
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Outcome;
use toolu_runtime::env::Env;

use super::{document, failure, flag, text};
use crate::finish::finish_issue;
use crate::herdr::Client;
use crate::journal::{self, Record};
use crate::launch::record::write_record;
use crate::launch::{Ctx as LaunchCtx, Herdr};
use crate::paths::Paths;
use crate::tracker::Gh;

pub(super) fn finish_command() -> Command {
  Command::new("finish")
    .about("Tear down one sub-issue after its PR merged")
    .arg(
      Arg::new("state-dir")
        .value_name("STATE_DIR")
        .required(true)
        .help("The epic state directory"),
    )
    .arg(
      Arg::new("key")
        .value_name("KEY")
        .required(true)
        .help("The issue key"),
    )
    .arg(flag(
      "abandon",
      "Tear down without a merged PR; keep the branch",
    ))
    .arg(flag("dry-run", "Validate and print the planned teardown"))
}

pub(super) fn release_command() -> Command {
  Command::new("release")
    .about("Release a coupling hold so the issue launches next")
    .arg(
      Arg::new("key")
        .value_name("KEY")
        .required(true)
        .help("The held issue key"),
    )
    .arg(flag("dry-run", "Find the hold and change nothing"))
}

/// Finish one issue; `cleanup-incomplete` exits 1 with the document, except
/// a dry run, which only plans.
pub(super) fn finish(matches: &ArgMatches, env: &Env) -> Outcome {
  let state = PathBuf::from(text(matches, "state-dir").unwrap_or(""));
  let key = text(matches, "key").unwrap_or("");
  let paths = Paths::from_env(env);
  let gh = Gh::new(env);
  let mut ctx = LaunchCtx {
    env,
    herdr: Herdr {
      client: Client::new(),
      paths: &paths,
      env,
    },
    gh: gh.as_ref().map_err(Clone::clone),
    session_wait: Duration::from_secs(10),
  };
  let flags = (matches.get_flag("abandon"), matches.get_flag("dry-run"));
  match finish_issue(&state, key, flags, &mut ctx) {
    Ok(report) => {
      let planned = report.finished.get("dry_run") == Some(&Value::Bool(true));
      let incomplete = !planned
        && report.finished.get("stage").and_then(Value::as_str) == Some("cleanup-incomplete");
      let mut outcome = Outcome::data(report.finished.to_string());
      outcome.exit = if incomplete {
        Exit::Failure
      } else {
        Exit::Success
      };
      outcome.stderr = (!report.warnings.is_empty()).then(|| report.warnings.join("\n"));
      outcome
    }
    Err(err) => failure("toolu epic finish", &err),
  }
}

/// The record of `key` in the registered epics.
fn find_record(paths: &Paths, key: &str) -> Result<PathBuf, String> {
  let registry = crate::disk::read_value(&paths.registry())?;
  let epics = registry
    .get("epics")
    .and_then(Value::as_array)
    .cloned()
    .unwrap_or_default();
  epics
    .iter()
    .filter_map(|epic| epic.get("state_dir").and_then(Value::as_str))
    .map(|dir| Path::new(dir).join("issues").join(format!("{key}.json")))
    .find(|path| path.is_file())
    .ok_or_else(|| format!("no registered issue {key}"))
}

/// Clear a coupling hold so the next launch skips the check: through the
/// engine when it runs, else in the registered epic's record.
pub(super) fn release(matches: &ArgMatches, env: &Env) -> Outcome {
  let key = text(matches, "key").unwrap_or("");
  let paths = Paths::from_env(env);
  if !matches.get_flag("dry-run") && crate::lock::live(&paths.lock()) && paths.socket().exists() {
    let request = json!({"op": "release", "key": key});
    return match crate::client::exchange_retry(&paths, crate::PROTOCOL, &request) {
      Ok(reply) => match reply.get("error").and_then(Value::as_str) {
        Some(err) => failure("toolu epic release", err),
        None => document(&reply),
      },
      Err(err) => failure("toolu epic release", &err),
    };
  }
  let path = match find_record(&paths, key) {
    Ok(path) => path,
    Err(err) => return failure("toolu epic release", &err),
  };
  let Ok(Value::Object(mut record)) = crate::disk::read_value(&path) else {
    let message = format!("invalid issue record {}", path.display());
    return failure("toolu epic release", &message);
  };
  let Some(hold) = record.get("hold").cloned().filter(|hold| !hold.is_null()) else {
    return failure("toolu epic release", &format!("{key} is not held"));
  };
  if !matches.get_flag("dry-run") {
    record.remove("hold");
    record.insert("coupling_released".to_owned(), json!(true));
    if let Err(err) = write_record(&path, &record) {
      return failure("toolu epic release", &err);
    }
    let note = hold.get("with").and_then(Value::as_str).unwrap_or("");
    let line = Record::new("action", "release", key, "", note);
    if let Err(err) = journal::append(
      &paths.journal_dir(),
      &paths.journal_lock(),
      &line,
      SystemTime::now(),
    ) {
      return failure("toolu epic release", &err);
    }
  }
  document(
    &json!({"key": key, "released": true, "hold": hold, "dry_run": matches.get_flag("dry-run")}),
  )
}

#[cfg(test)]
#[path = "tests/teardown_test.rs"]
mod tests;
