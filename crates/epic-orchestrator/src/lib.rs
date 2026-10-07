//! The epic-orchestrator plugin's crate: the `toolu epic` namespace, not ported yet
//! (#434, #435, #448).

use clap::{ArgMatches, Command};
use serde_json::json;
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::config::secrets;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::namespace::Planned;

/// The plugin this crate belongs to: `plugins/epic-orchestrator`.
pub const PLUGIN: &str = "epic-orchestrator";

const NAMESPACE: Planned = Planned {
  name: "epic",
  about: "Drive an epic to merged PRs: the resident engine, merge queue and trackers",
  verbs: &[
    "engine", "start", "status", "pause", "resume", "ack", "answer", "wait", "report", "job",
    "graph", "route", "launch", "finish", "close", "release", "jira", "probe", "gate", "queue",
  ],
  issues: &[434, 435, 448],
};

/// The `toolu epic` namespace.
pub fn command() -> Command {
  NAMESPACE.command().subcommand(
    Command::new("token")
      .about("Manage the status bearer token")
      .subcommand_required(true)
      .arg_required_else_help(true)
      .subcommand(Command::new("new").about("Rotate the status bearer token in secrets.json")),
  )
}

fn token_new(ctx: &Ctx) -> Outcome {
  let mut env = Env::process();
  if let Some(dir) = &ctx.config_dir {
    env = env.with("TOOLU_CONFIG_DIR", &dir.to_string_lossy());
  }
  let roots = Roots::new(env, ctx.host);
  match secrets::rotate_status_token(&roots) {
    Ok(path) if ctx.json => Outcome::data(
      json!({ "status_token": "<redacted>", "path": path, "rotated": true }).to_string(),
    ),
    Ok(path) => Outcome::data(format!(
      "toolu epic: status token rotated at {}",
      path.display()
    )),
    Err(error) => Outcome::failed(Exit::Failure, format!("toolu epic token new: {error}")),
  }
}

/// Run `toolu epic token new` or the planned-verb placeholder.
pub fn run(matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  match matches.subcommand() {
    Some(("token", token)) if token.subcommand_name() == Some("new") => token_new(ctx),
    _ => NAMESPACE.run(ctx),
  }
}

#[cfg(test)]
#[path = "tests/lib_test.rs"]
mod tests;
