//! `toolu opencode`: what the generated `OpenCode` plugin shim calls. `bridge`
//! runs one plugin's `hooks.json` entries for one `OpenCode` hook call read from
//! stdin and prints the folded answer; `catalog` prints the plugin's skills and
//! commands in the shape `OpenCode` registers them. Both take the plugin's root,
//! which the shim passes as its own directory.

use std::path::{Path, PathBuf};

use clap::{Arg, ArgMatches, Command};
use toolu_protocol::exit::Exit;
use toolu_protocol::opencode::Hook;
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::env::Env;
use toolu_runtime::invocation::current_exe;

mod bridge;
mod catalog;

/// The `toolu opencode` namespace.
pub fn command() -> Command {
  Command::new("opencode")
    .about("Run toolu plugins under OpenCode; their generated server.js calls these")
    .subcommand_required(true)
    .arg_required_else_help(true)
    .subcommand(
      Command::new("bridge")
        .about("Run a plugin's hooks for one OpenCode hook call read from stdin")
        .arg(
          Arg::new("event")
            .long("event")
            .value_name("HOOK")
            .required(true)
            .value_parser(Hook::ALL.map(Hook::name))
            .help("The OpenCode hook the call came from"),
        )
        .arg(plugin_root()),
    )
    .subcommand(
      Command::new("catalog")
        .about("Print a plugin's skills and commands as OpenCode registers them")
        .arg(plugin_root()),
    )
}

fn plugin_root() -> Arg {
  Arg::new("plugin-root")
    .long("plugin-root")
    .value_name("DIR")
    .required(true)
    .value_parser(clap::value_parser!(PathBuf))
    .help("The plugin directory: the shim's own")
}

/// Whether the verb reads stdin: `bridge` reads the hook call.
pub fn needs_stdin(matches: &ArgMatches) -> bool {
  matches.subcommand_name() == Some("bridge")
}

/// Run `toolu opencode <verb>`.
pub fn run(matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  let root = |sub: &ArgMatches| {
    sub
      .get_one::<PathBuf>("plugin-root")
      .cloned()
      .unwrap_or_default()
  };
  match matches.subcommand() {
    Some(("bridge", sub)) => {
      let hook = sub
        .get_one::<String>("event")
        .and_then(|name| Hook::parse(name))
        .unwrap_or(Hook::ToolBefore);
      let call = bridge::Bridge {
        hook,
        root: &root(sub),
        exe: current_exe().unwrap_or_else(|| PathBuf::from("toolu")),
        env: Env::process(),
      };
      // The shim reads the answer, a failure included, from stdout.
      Outcome::data(call.run(ctx.stdin.as_deref()))
    }
    Some(("catalog", sub)) => catalog_outcome(&root(sub)),
    _ => Outcome::failed(Exit::Usage, "toolu opencode: a verb is required".to_owned()),
  }
}

fn catalog_outcome(root: &Path) -> Outcome {
  match catalog::catalog(root) {
    Ok(doc) => Outcome::data(doc.to_string()),
    Err(err) => Outcome::failed(Exit::Failure, format!("toolu opencode catalog: {err}")),
  }
}

#[cfg(test)]
#[path = "tests/opencode_test.rs"]
mod tests;
