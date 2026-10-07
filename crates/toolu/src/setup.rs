//! `toolu setup agents`: Codex profiles, the same plan as `setup.ts` (#445).

use clap::{Arg, ArgAction, ArgMatches, Command};
use toolu_protocol::host::Host;
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::env::Env;

mod agents;

/// Preview, install or remove toolu's agent profiles.
pub fn command() -> Command {
  Command::new("setup")
    .about("Preview, install or remove toolu's agent profiles")
    .subcommand(
      Command::new("agents")
        .about("Preview, install or remove Codex agent profiles")
        .arg(
          Arg::new("args")
            .num_args(0..)
            .action(ArgAction::Append)
            .allow_hyphen_values(true)
            .trailing_var_arg(true)
            .help("preview, install [--force], or remove --yes [--force]"),
        ),
    )
}

/// Run `agents`. Any other verb is the script's usage exit.
pub fn run(matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  let Some((_, sub)) = matches.subcommand() else {
    return agents::usage();
  };
  let args: Vec<String> = sub
    .get_many::<String>("args")
    .map(|values| values.cloned().collect())
    .unwrap_or_default();
  agents::run(ctx, &args)
}

/// Process environment, with `--host` winning over `TOOLU_HOST_OVERRIDE` only
/// when the flag is set. The script reads the variable; the CLI flag is the same switch.
pub(super) fn env_of(ctx: &Ctx) -> Env {
  let mut env = Env::process();
  if let Some(host) = ctx.host {
    env = env.with("TOOLU_HOST_OVERRIDE", host.name());
  }
  env
}

/// Whether this run is `OpenCode`.
pub(super) fn opencode(ctx: &Ctx, env: &Env) -> bool {
  ctx.host == Some(Host::Opencode) || env.get("TOOLU_HOST_OVERRIDE") == Some("opencode")
}

#[cfg(test)]
#[path = "tests/setup_test.rs"]
mod tests;
