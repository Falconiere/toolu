//! One `toolu babysit` verb: its matches, run against the world the binary
//! hands it (environment, host, GitHub and Jev settings, stdin), printed as
//! one JSON document or the structured error.

use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use clap::ArgMatches;
use toolu_protocol::host::Host;
use toolu_runtime::cli::Outcome;
use toolu_runtime::env::Env;
use toolu_state::time::{iso_seconds, parse_iso};

use crate::error::{Failure, Result, finish};
use crate::verdict;

mod github;
mod local;

/// What a verb runs against.
pub(crate) struct World<'a> {
  /// The environment of every call and command.
  pub(crate) env: &'a Env,
  /// The controlling host: `--host`, else detected.
  pub(crate) host: Host,
  /// The GitHub client's settings, or why `PB_GH_*` is invalid.
  pub(crate) github: std::result::Result<toolu_github::Config, toolu_github::Error>,
  /// The Jev client's settings.
  pub(crate) jev: toolu_jev_client::Config,
  /// Stdin, for `parse-verdict`.
  pub(crate) stdin: Option<&'a str>,
  /// The `toolu` binary a fixer reports with.
  pub(crate) toolu: &'a Path,
  /// The working directory, for the project config.
  pub(crate) cwd: Option<&'a Path>,
  /// The pause between `resolve` confirmations.
  pub(crate) pause: Duration,
}

/// Runs the verb in `matches`.
pub(crate) fn execute(matches: &ArgMatches, world: &World<'_>) -> Outcome {
  let Some((name, sub)) = matches.subcommand() else {
    return Failure::usage("a babysit verb is required").outcome();
  };
  if name == "collect" {
    return github::collect(sub, world);
  }
  finish(match name {
    "tick" => github::tick(sub, world),
    "wait" => github::wait(sub, world),
    "reply" => github::reply(sub, world),
    "resolve" => github::resolve(sub, world),
    "reduce-state" => local::reduce_state(sub),
    "parse-verdict" => Ok(verdict::parse(world.stdin.unwrap_or_default())),
    "record" => local::record(sub, world),
    "route-fix" => local::route_fix(sub, world),
    "dispatch-fix" => local::dispatch_fix(sub, world),
    "fixer-report" => local::fixer_report(sub),
    "stop" => local::stop(sub, world),
    other => Err(Failure::usage(format!("unknown babysit verb: {other}"))),
  })
}

/// A string argument.
fn text<'m>(matches: &'m ArgMatches, id: &str) -> Option<&'m str> {
  matches.get_one::<String>(id).map(String::as_str)
}

/// A path argument.
fn path<'m>(matches: &'m ArgMatches, id: &str) -> Option<&'m Path> {
  text(matches, id).map(Path::new)
}

/// A required path argument.
fn required<'m>(matches: &'m ArgMatches, id: &str) -> Result<&'m Path> {
  path(matches, id).ok_or_else(|| Failure::usage(format!("--{id} <path> required")))
}

/// A number argument.
fn number(matches: &ArgMatches, id: &str) -> Option<u64> {
  matches.get_one::<u64>(id).copied()
}

/// Now, to the second.
fn now() -> String {
  iso_seconds(SystemTime::now())
}

/// `text` as Unix seconds, when it is `YYYY-MM-DDTHH:MM:SSZ`.
fn seconds(text: &str) -> Result<u64> {
  parse_iso(text)
    .and_then(|at| at.duration_since(UNIX_EPOCH).ok())
    .map(|since| since.as_secs())
    .ok_or_else(|| Failure::usage("--now must be YYYY-MM-DDTHH:MM:SSZ"))
}

#[cfg(test)]
#[path = "tests/verbs_test.rs"]
mod tests;
