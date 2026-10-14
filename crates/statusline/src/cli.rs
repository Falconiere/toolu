//! The three `toolu statusline` verbs.

use clap::{Arg, ArgAction, Command};

/// The public verbs and their flags.
pub(crate) fn command() -> Command {
  Command::new("statusline")
    .about("Render and set up the native Claude status line")
    .subcommand_required(true)
    .arg_required_else_help(true)
    .subcommand(
      Command::new("render").about("Render one Claude status-line JSON payload from stdin"),
    )
    .subcommand(
      Command::new("setup")
        .about("Wire Claude settings.json to the native renderer")
        .arg(
          Arg::new("force")
            .short('f')
            .long("force")
            .action(ArgAction::SetTrue)
            .help("Replace an existing custom statusLine after backing it up"),
        ),
    )
    .subcommand(
      Command::new("refresh").about("Migrate old settings and retire the old statusline path"),
    )
    .subcommand(
      Command::new("refresh-cache")
        .about("Refresh cached Git counts for a statusline worktree (internal)")
        .arg(Arg::new("worktree").value_name("WORKTREE").required(true)),
    )
}

#[cfg(test)]
#[path = "tests/cli_test.rs"]
mod tests;
