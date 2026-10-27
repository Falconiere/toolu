//! The `toolu epic notify` verb: a worker notice for the orchestrator, or
//! one sibling issue.

use clap::{Arg, Command};

/// The notice kinds a worker sends.
pub(crate) const NOTIFY_KINDS: [&str; 5] =
  ["blocked", "unblocked", "contract-changed", "stuck", "help"];

/// The complete `toolu epic notify` command tree.
pub(crate) fn command() -> Command {
  Command::new("notify")
    .about("Notify the orchestrator, or one sibling issue")
    .arg(
      Arg::new("kind")
        .long("kind")
        .value_name("KIND")
        .required(true)
        .value_parser(NOTIFY_KINDS)
        .help("The notice kind"),
    )
    .arg(
      Arg::new("status-file")
        .long("status-file")
        .value_name("FILE")
        .required(true)
        .help("The issue status snapshot"),
    )
    .arg(
      Arg::new("note")
        .long("note")
        .value_name("TEXT")
        .help("Note stored with the notice"),
    )
    .arg(
      Arg::new("to")
        .long("to")
        .value_name("KEY")
        .help("One sibling issue key; the orchestrator only when omitted"),
    )
}

#[cfg(test)]
#[path = "tests/verbs_notify_test.rs"]
mod tests;
