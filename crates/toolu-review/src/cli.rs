//! The `toolu review` command tree, including the old writer's arguments.

use clap::{Arg, Command};

fn option(name: &'static str, help: &'static str) -> Arg {
  Arg::new(name).long(name).value_name("VALUE").help(help)
}

/// The review namespace and its state writer.
pub(crate) fn command() -> Command {
  Command::new("review")
    .about("Pre-push code review state for the native push-review gate")
    .subcommand_required(true)
    .arg_required_else_help(true)
    .subcommand(
      Command::new("write-state")
        .about("Record a review of the committed branch diff")
        .arg(
          option(
            "findings-count",
            "Number of open findings (zero for a clean review)",
          )
          .required(true),
        )
        .arg(option(
          "reviewers",
          "JSON list of reviewers; defaults to toolu-review:review",
        ))
        .arg(option("findings", "JSON list of findings; defaults to []"))
        .arg(option(
          "repo",
          "Repository or worktree to review; defaults to the current directory",
        ))
        .arg(option("branch", "Target branch for a detached worktree"))
        .arg(option(
          "reviewed-files",
          "Comma-separated reviewed paths instead of the Git diff",
        )),
    )
}

#[cfg(test)]
#[path = "tests/cli_test.rs"]
mod tests;
