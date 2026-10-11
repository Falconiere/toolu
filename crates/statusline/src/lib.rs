//! The statusline plugin: Claude's status bar and its native setup and refresh commands.

use clap::{ArgMatches, Command};
use std::path::Path;
use toolu_engine::status::StatusSnapshot;
use toolu_protocol::exit::Exit;
use toolu_protocol::host::Host;
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;

mod cli;
mod collect;
mod payload;
mod refresh;
mod render;
mod setup;

/// The plugin this crate belongs to: `plugins/statusline`.
pub const PLUGIN: &str = "statusline";

/// The `toolu statusline` namespace.
pub fn command() -> Command {
  cli::command()
}

/// Whether the selected command consumes Claude's JSON payload on stdin.
pub fn needs_stdin(matches: &ArgMatches) -> bool {
  matches.subcommand_name() == Some("render")
}

/// Render the host's exact no-flag refresh command without building the CLI tree.
pub fn render(stdin: Option<String>, status: &dyn StatusSnapshot) -> Outcome {
  let roots = Roots::new(Env::process(), None);
  let ctx = Ctx {
    stdin,
    json: false,
    quiet: false,
    host: None,
    config_dir: None,
  };
  render::run(&ctx, &roots, status)
}

/// Run the selected native statusline verb with the hub's status snapshot.
pub fn run(matches: &ArgMatches, ctx: &Ctx, status: &dyn StatusSnapshot) -> Outcome {
  let mut env = Env::process();
  if let Some(dir) = &ctx.config_dir {
    env = env.with("TOOLU_CONFIG_DIR", &dir.to_string_lossy());
  }
  let roots = Roots::new(env, ctx.host);
  match matches.subcommand() {
    Some(("render", _)) => render::run(ctx, &roots, status),
    Some(("setup", args)) => setup::run(
      &Roots::new(roots.env().clone(), Some(Host::Claude)),
      args.get_flag("force"),
      ctx.json,
    ),
    Some(("refresh", _)) => refresh::run(&roots, ctx.json),
    Some(("refresh-cache", args)) => {
      let Some(dir) = args.get_one::<String>("worktree") else {
        return Outcome::failed(Exit::Usage, "statusline: worktree is required".to_owned());
      };
      match status.refresh_cache(&roots, Path::new(dir)) {
        Ok(()) => Outcome {
          exit: Exit::Success,
          stdout: ctx.json.then(|| "{}".to_owned()),
          stderr: None,
        },
        Err(error) => Outcome::failed(Exit::Failure, error.to_string()),
      }
    }
    _ => Outcome::failed(Exit::Usage, "statusline: a verb is required".to_owned()),
  }
}

/// The native `SessionStart` hook; it refreshes the shim and migrates old settings.
pub fn session_start(env: &Env, plugin_root: Option<&Path>) -> Outcome {
  refresh::session_start(&Roots::new(env.clone(), None), plugin_root)
}

#[cfg(test)]
#[path = "tests/lib_test.rs"]
mod tests;
