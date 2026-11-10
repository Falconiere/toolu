//! The `toolu epic report` and `toolu epic job` command definitions.

use clap::{Arg, Command};

use crate::verbs::flag;

/// The phases a worker reports, in pipeline order.
const PHASES: [&str; 12] = [
  "brainstorm",
  "spec",
  "spec-review",
  "plan",
  "plan-review",
  "execution",
  "pr-open",
  "babysit",
  "rebasing",
  "ready",
  "needs-human",
  "failed",
];

/// `toolu epic report <phase>`: record a worker phase.
pub(crate) fn report_command() -> Command {
  Command::new("report")
    .about("Record a worker phase")
    .arg(
      Arg::new("phase")
        .value_name("PHASE")
        .required(true)
        .value_parser(PHASES)
        .help("The phase to record"),
    )
    .arg(
      Arg::new("status-file")
        .long("status-file")
        .value_name("FILE")
        .required(true)
        .help("The issue status snapshot"),
    )
    .arg(
      Arg::new("pr")
        .long("pr")
        .value_name("N")
        .value_parser(clap::value_parser!(u64))
        .help("Pull request number"),
    )
    .arg(
      Arg::new("note")
        .long("note")
        .value_name("TEXT")
        .help("Note stored with the phase"),
    )
}

/// `toolu epic job -- <argv>`: run a command under the worktree's lease.
pub(crate) fn job_command() -> Command {
  Command::new("job")
    .about("Run a command under the worktree's resource lease, waiting up to 30 min for admission")
    .arg(flag(
      "no-wait",
      "Refuse at once when capacity or pressure holds instead of waiting",
    ))
    .arg(
      Arg::new("argv")
        .value_name("ARGV")
        .required(true)
        .num_args(1..)
        .trailing_var_arg(true)
        .allow_hyphen_values(true)
        .help("The command and its arguments"),
    )
}

#[cfg(test)]
#[path = "tests/verbs_report_test.rs"]
mod tests;
