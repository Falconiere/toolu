//! The verbs that stay on this machine: `reduce-state`, `record`,
//! `route-fix`, `dispatch-fix`, `fixer-report` and `stop`.

use std::path::Path;
use std::time::SystemTime;

use clap::ArgMatches;
use serde_json::{Value, json};
use toolu_runtime::host::roots::Roots;

use super::{World, now, path, required, seconds, text};
use crate::dispatch::dispatcher::Options;
use crate::dispatch::outcome::report;
use crate::dispatch::start::Start;
use crate::dispatch::{Verb, dispatch_fix as run_dispatch};
use crate::error::{Failure, Result};
use crate::fixer::config;
use crate::fixer::host::Fixer;
use crate::record::{Entry, record as run_record};
use crate::reduce::{Paths, reduce};
use crate::route::jev::Scoring;
use crate::route::{Request, route};
use crate::state::{sibling, write_json};
use crate::tick::valid;
use crate::writes::Slot;

fn json_file(file: &Path, missing: &str, invalid: &str, code: &'static str) -> Result<Value> {
  let shown = file.display();
  let text = std::fs::read_to_string(file)
    .map_err(|_error| Failure::new(code, format!("{missing}: {shown}")))?;
  serde_json::from_str(&text).map_err(|_error| Failure::new(code, format!("{invalid}: {shown}")))
}

/// The previous state for `snapshot`, when `--state` names an existing file.
fn previous(file: Option<&Path>, snapshot: &Value) -> Result<Option<Value>> {
  let Some(file) = file.filter(|file| file.exists()) else {
    return Ok(None);
  };
  let malformed = "state file is not valid JSON";
  let state = json_file(file, malformed, malformed, "state_malformed")
    .map_err(|failure| failure.with("source", "state"))?;
  if state.get("version").and_then(Value::as_u64) != Some(2) {
    let message = format!("state file is not version 2: {}", file.display());
    let version = state.get("version").cloned().unwrap_or(Value::Null);
    return Err(Failure::new("state_malformed", message).with("version", version));
  }
  let owner = |value: &Value| json!({"repo": value.get("repo"), "number": value.get("number")});
  if owner(&state) != owner(snapshot) {
    let message = format!(
      "state belongs to {}, snapshot is {}",
      owner(&state),
      owner(snapshot)
    );
    let failure = Failure::new("slot_mismatch", message).with("source", "state");
    return Err(
      failure
        .with("state", owner(&state))
        .with("snapshot", owner(snapshot)),
    );
  }
  Ok(Some(state))
}

/// Reduce a snapshot against a state: `{state, result}`, written where asked.
pub(super) fn reduce_state(matches: &ArgMatches) -> Result<Value> {
  let file = required(matches, "snapshot")?;
  let now = text(matches, "now").unwrap_or_default();
  seconds(now).map_err(|_failure| {
    Failure::usage("--now must be an ISO-8601 UTC timestamp (YYYY-MM-DDTHH:MM:SSZ)")
  })?;
  let snapshot = json_file(
    file,
    "snapshot not found",
    "snapshot is not valid JSON",
    "invalid_json",
  )
  .map_err(|failure| failure.with("source", "snapshot"))?;
  valid(&snapshot)?;
  let state_file = path(matches, "state");
  let previous = previous(state_file, &snapshot)?;
  let shown = |id: &str, fallback: Option<&Path>| {
    path(matches, id)
      .or(fallback)
      .map(|path| path.display().to_string())
      .unwrap_or_default()
  };
  let state = shown("state-path", state_file);
  let snapshot_path = shown("snapshot-path", Some(file));
  let result = sibling(Path::new(&state), ".result.json")
    .display()
    .to_string();
  let paths = Paths {
    state: &state,
    snapshot: &snapshot_path,
    result: &result,
  };
  let reduced = reduce(&snapshot, previous.as_ref(), now, &paths);
  if let Some(out) = path(matches, "state-out") {
    write_json(out, &reduced.state, "state_out")?;
  }
  if let Some(out) = path(matches, "result-out") {
    write_json(out, &reduced.result, "result_out")?;
  }
  Ok(json!({"state": reduced.state, "result": reduced.result}))
}

/// `record flag-injection|round|status`.
pub(super) fn record(matches: &ArgMatches, world: &World<'_>) -> Result<Value> {
  let Some((name, sub)) = matches.subcommand() else {
    return Err(Failure::usage(
      "record: subcommand must be flag-injection, round or status",
    ));
  };
  let entry = match name {
    "flag-injection" => match text(sub, "thread").filter(|thread| !thread.is_empty()) {
      Some(thread) => Entry::FlagInjection(thread),
      None => {
        return Err(Failure::usage(
          "record flag-injection: --thread <graphqlId> required",
        ));
      }
    },
    "round" => Entry::Round {
      had_rejection: text(sub, "had-rejection") == Some("true"),
      fix_pushed: sub.get_flag("fix-pushed"),
    },
    _ => Entry::Status(text(sub, "status").unwrap_or_default()),
  };
  let now = now();
  let slot = Slot {
    path: required(sub, "state-file")?,
    env: world.env,
    now: &now,
  };
  run_record(&slot, &entry)
}

/// Route fix items to fixer hosts.
pub(super) fn route_fix(matches: &ArgMatches, world: &World<'_>) -> Result<Value> {
  let controller = Fixer::parse(world.host.name())?;
  let roots = Roots::new(world.env.clone(), Some(world.host));
  let settings = config::load(&roots, world.cwd, controller)?;
  let now = match text(matches, "now") {
    Some(text) => seconds(text)?,
    None => seconds(&toolu_state::time::iso_seconds(SystemTime::now()))?,
  };
  let raise: Vec<String> = matches
    .get_many::<String>("raise")
    .into_iter()
    .flatten()
    .cloned()
    .collect();
  let request = Request {
    items: required(matches, "items")?,
    state_file: path(matches, "state-file"),
    raise: &raise,
    now,
  };
  let scoring = match path(matches, "jev-answers-in") {
    Some(answers) => Scoring::Replay(answers),
    None if matches.get_flag("no-jev") || !settings.jev => Scoring::Off,
    None => Scoring::Ask(world.env, world.jev.clone()),
  };
  route(&request, &settings, world.env, scoring)
}

fn options<'a>(matches: &'a ArgMatches, world: &'a World<'_>) -> Result<Options<'a>> {
  let dry = matches
    .try_get_one::<bool>("dry-run")
    .ok()
    .flatten()
    .copied()
    .unwrap_or(false);
  Ok(Options {
    state_file: required(matches, "state-file")?,
    env: world.env,
    toolu: world.toolu,
    dry,
  })
}

/// `dispatch-fix start|wait|cleanup`.
pub(super) fn dispatch_fix(matches: &ArgMatches, world: &World<'_>) -> Result<Value> {
  let Some((name, sub)) = matches.subcommand() else {
    return Err(Failure::usage(
      "dispatch-fix: subcommand must be start, wait or cleanup",
    ));
  };
  let options = options(sub, world)?;
  match name {
    "start" => {
      let start = Start {
        plan: required(sub, "plan")?,
        items: required(sub, "items")?,
        repo_root: text(sub, "repo-root").unwrap_or_default(),
        branch: text(sub, "branch").unwrap_or_default(),
        base: text(sub, "base").unwrap_or_default(),
      };
      run_dispatch(&Verb::Start(start), &options)
    }
    "wait" => run_dispatch(&Verb::Wait(text(sub, "timeout-seconds")), &options),
    _ => run_dispatch(&Verb::Cleanup, &options),
  }
}

/// A fixer's report on its group.
pub(super) fn fixer_report(matches: &ArgMatches) -> Result<Value> {
  let file = required(matches, "report")?;
  report(
    file,
    text(matches, "status").unwrap_or_default(),
    text(matches, "note"),
    &now(),
  )
}

/// End the fixer round, then record the loop cancelled.
pub(super) fn stop(matches: &ArgMatches, world: &World<'_>) -> Result<Value> {
  let cleaned = run_dispatch(&Verb::Cleanup, &options(matches, world)?)?;
  let now = now();
  let slot = Slot {
    path: required(matches, "state-file")?,
    env: world.env,
    now: &now,
  };
  let recorded = run_record(&slot, &Entry::Status("cancelled"))?;
  Ok(json!({"version": 1, "cleanup": cleaned, "status": recorded}))
}

#[cfg(test)]
#[path = "tests/local_test.rs"]
mod tests;
