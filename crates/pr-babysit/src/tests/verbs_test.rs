use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;
use toolu_protocol::exit::Exit;
use toolu_protocol::host::Host;
use toolu_runtime::env::Env;

use super::{World, execute, seconds};
use crate::fixture;

/// A world on Claude over `env`, with GitHub settings from `env`.
pub(super) fn world<'a>(env: &'a Env, stdin: Option<&'a str>) -> World<'a> {
  World {
    env,
    host: Host::Claude,
    github: toolu_github::Config::one_shot(env),
    jev: toolu_jev_client::Config::default(),
    stdin,
    toolu: Path::new("/opt/toolu/bin/toolu"),
    cwd: None,
    pause: Duration::ZERO,
  }
}

/// The JSON document at `path`.
pub(super) fn read(path: &Path) -> Value {
  serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// A copy of the recorded PR 165 initial state in `dir`.
pub(super) fn initial_state(dir: &Path) -> PathBuf {
  let path = dir.join("falconiere-toolu-165.json");
  std::fs::copy(fixture::path("states/toolu-165-initial.json"), &path).unwrap();
  path
}

/// `toolu babysit <args>` in `world`: its exit and stdout document.
pub(super) fn run(args: &[&str], world: &World<'_>) -> (Exit, Value) {
  let line = std::iter::once("babysit").chain(args.iter().copied());
  let matches = crate::cli::command().try_get_matches_from(line).unwrap();
  let outcome = execute(&matches, world);
  let stdout = outcome.stdout.unwrap_or_default();
  (
    outcome.exit,
    serde_json::from_str(&stdout).unwrap_or(Value::String(stdout)),
  )
}

#[test]
fn parse_verdict_reads_the_comment_from_stdin() {
  let env = Env::default();
  let comment = fixture::text("verdicts/pr122-verdict-approved.txt");
  let (exit, parsed) = run(&["parse-verdict"], &world(&env, Some(&comment)));
  assert_eq!(exit, Exit::Success);
  assert_eq!(parsed, crate::verdict::parse(&comment));
  assert_eq!(parsed["verdict"], "approved");
}

#[test]
fn parse_verdict_without_stdin_parses_nothing() {
  let env = Env::default();
  let (exit, parsed) = run(&["parse-verdict"], &world(&env, None));
  assert_eq!(exit, Exit::Success);
  assert_eq!(parsed, crate::verdict::parse(""));
}

#[test]
fn a_timestamp_is_whole_utc_seconds() {
  assert_eq!(seconds("1970-01-01T00:03:00Z").unwrap(), 180);
  let failure = seconds("yesterday").unwrap_err();
  assert_eq!(failure.code, "usage");
  assert_eq!(failure.message, "--now must be YYYY-MM-DDTHH:MM:SSZ");
}
