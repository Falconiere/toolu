//! `toolu epic hosts pause|resume <host>`: take one agent host out of
//! rotation, or bring it back (which also clears its usage-limit cooldown).

use clap::{Arg, ArgMatches, Command};
use serde_json::json;
use toolu_runtime::cli::Outcome;
use toolu_runtime::env::Env;

use crate::hosts::host_kind;
use crate::query::ask_checked;
use crate::verbs::{failed, text};

/// The `hosts` subcommand tree.
pub(crate) fn command() -> Command {
  Command::new("hosts")
    .about("Pause or resume an agent host")
    .subcommand_required(true)
    .arg_required_else_help(true)
    .subcommand(host_command(
      "pause",
      "Stop sending new agents to HOST; running workers keep running",
    ))
    .subcommand(host_command(
      "resume",
      "Return HOST to rotation and clear its usage-limit cooldown",
    ))
}

fn host_command(name: &'static str, about: &'static str) -> Command {
  Command::new(name).about(about).arg(
    Arg::new("host")
      .value_name("HOST")
      .required(true)
      .help("The host: claude, codex, cursor or muse"),
  )
}

/// Dispatch `hosts pause` or `hosts resume`.
pub(crate) fn run(matches: &ArgMatches, env: &Env) -> Outcome {
  let (name, args) = matches.subcommand().unwrap_or(("pause", matches));
  let pausing = name != "resume";
  let verb = if pausing {
    "toolu epic hosts pause"
  } else {
    "toolu epic hosts resume"
  };
  let kind = match text(args, "host").map(host_kind) {
    Some(Ok(kind)) => kind,
    Some(Err(err)) => return failed(verb, &err),
    None => return failed(verb, "a host is required"),
  };
  let op = if pausing { "host-pause" } else { "host-resume" };
  let request = json!({"op": op, "host": kind.name(), "by": "user"});
  ask_checked(env, if pausing { "hosts pause" } else { "hosts resume" }, &request)
}

#[cfg(test)]
#[path = "tests/verbs_hosts_test.rs"]
mod tests;
