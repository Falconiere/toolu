//! The clap tree of `toolu sandbox`. Nothing in it depends on the machine, so the
//! generated reference is the same on macOS and Linux.

use clap::{Arg, ArgAction, ArgMatches, Command};

/// The namespace and its verbs.
pub(crate) fn command() -> Command {
  Command::new("sandbox")
    .about(
      "Run builds, gates and agent CLIs in a Linux sandbox: a Lima VM on macOS, this machine on Linux",
    )
    .subcommand_required(true)
    .arg_required_else_help(true)
    .subcommand(Command::new("setup").about(
      "Create and provision the sandbox, then print how to install and log in to the agent CLIs",
    ))
    .subcommand(
      Command::new("status").about("Show the backend, the sandbox's state, its settings and this project"),
    )
    .subcommand(
      Command::new("start")
        .about("Start the sandbox, applying its settings and this project's mounts")
        .arg(flag(
          "restart",
          "Stop a running sandbox first; every command running in it stops",
        )),
    )
    .subcommand(Command::new("stop").about("Stop the sandbox"))
    .subcommand(
      Command::new("delete")
        .about("Delete the sandbox with its disk, caches and agent CLI logins")
        .arg(flag("yes", "Confirm the deletion").required(true)),
    )
    .subcommand(
      Command::new("run")
        .about("Run a command in the sandbox at this directory; the exit status is the command's")
        .arg(flag(
          "clone",
          "Run in a clone of this checkout on the sandbox's own disk, with its uncommitted and untracked changes",
        ))
        .arg(
          Arg::new("argv")
            .value_name("COMMAND")
            .required(true)
            .num_args(1..)
            .trailing_var_arg(true)
            .allow_hyphen_values(true)
            .help("The command and its arguments"),
        ),
    )
    .subcommand(Command::new("shell").about("Open a login shell in the sandbox at this directory"))
    .subcommand(
      Command::new("prune")
        .about("Report the clones of checkouts that no longer exist; --apply removes them")
        .arg(flag("apply", "Remove the orphan clones with their caches")),
    )
}

/// The command a `run` was given.
pub(crate) fn argv(verb: &ArgMatches) -> Vec<String> {
  verb
    .try_get_many::<String>("argv")
    .ok()
    .flatten()
    .map(|words| words.cloned().collect())
    .unwrap_or_default()
}

/// Whether a verb's boolean `name` flag is set.
pub(crate) fn set(verb: &ArgMatches, name: &str) -> bool {
  verb
    .try_get_one::<bool>(name)
    .ok()
    .flatten()
    .copied()
    .unwrap_or(false)
}

fn flag(name: &'static str, help: &'static str) -> Arg {
  Arg::new(name)
    .long(name)
    .action(ArgAction::SetTrue)
    .help(help)
}

#[cfg(test)]
#[path = "tests/cli_test.rs"]
mod tests;
