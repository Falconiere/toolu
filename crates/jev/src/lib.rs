//! The jev plugin's crate: `toolu jev noul|choice|score|ask`, typed judgments
//! from the Jev client of `toolu-jev-client` (#430). `cli` is the clap tree,
//! `call` turns its matches into a state and questions, and `present` turns the
//! reply or the error into an `Outcome`. This crate never reads stdin: the binary
//! hands it over when `needs_stdin` says a verb wants it.

use clap::{ArgMatches, Command};
use toolu_jev_client::{Config, Jev};
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::env::Env;

mod call;
mod cli;
mod present;

/// The plugin this crate belongs to: `plugins/jev`.
pub const PLUGIN: &str = "jev";

/// The `toolu jev` namespace.
pub fn command() -> Command {
  cli::command()
}

/// Run a `toolu jev` verb against Jev's endpoint with the process environment.
/// It reads no stdin; `needs_stdin` tells the caller when `execute` needs it.
pub fn run(matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  execute(matches, ctx, &Env::process(), Config::default(), None)
}

/// Whether the verb in `matches` reads stdin: `--state -`, or `ask -`.
pub fn needs_stdin(matches: &ArgMatches) -> bool {
  call::needs_stdin(matches)
}

/// Run a `toolu jev` verb with an explicit environment, endpoint and stdin text.
/// The key is checked first, so no argument or file is read without one.
pub fn execute(
  matches: &ArgMatches,
  _ctx: &Ctx,
  env: &Env,
  config: Config,
  stdin: Option<&str>,
) -> Outcome {
  let jev = match Jev::from_env(env, config) {
    Ok(jev) => jev,
    Err(err) => return present::error(&err),
  };
  let call = match call::build(matches, stdin) {
    Ok(call) => call,
    Err(err) => return present::call_error(&err),
  };
  match jev.ask(&call.state, &call.model, &call.questions) {
    Ok(reply) => present::reply(&reply, call.raw),
    Err(err) => present::error(&err),
  }
}

#[cfg(test)]
#[path = "tests/lib_test.rs"]
mod tests;
