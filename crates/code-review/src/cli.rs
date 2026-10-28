//! The clap tree of `toolu code-review`.

use clap::builder::PossibleValuesParser;
use clap::{Arg, ArgAction, ArgMatches, Command};

use crate::budget::Effort;

/// The namespace and its `run` verb.
pub(crate) fn command() -> Command {
  Command::new("code-review")
    .about("Native review of the push diff: anchored findings, coverage and a computed verdict")
    .subcommand_required(true)
    .arg_required_else_help(true)
    .subcommand(
      Command::new("run")
        .about("Review the branch diff and print the verdict report")
        .arg(
          Arg::new("base")
            .long("base")
            .value_name("REF")
            .help("The base to diff against; default: the push-review base"),
        )
        .arg(
          Arg::new("path")
            .long("path")
            .value_name("PATH")
            .action(ArgAction::Append)
            .help(
              "Review only this path; repeatable. A path-scoped run never writes passing state",
            ),
        )
        .arg(
          Arg::new("effort")
            .long("effort")
            .value_name("LEVEL")
            .value_parser(PossibleValuesParser::new(Effort::ALL.map(Effort::name)))
            .help(
              "Review effort: auto lets Jev pick the level; a fixed level skips it; default: codeReview.effort, else auto",
            ),
        ),
    )
    .subcommand(Command::new("dismiss")
      .about("Settle a branch finding by exact ID without a model call")
      .arg(Arg::new("finding-id").required(true))
      .arg(Arg::new("reason").long("reason").required(true).value_name("TEXT")))
}

/// The `--effort` of a `run`, when given.
pub(crate) fn effort(verb: &ArgMatches) -> Option<Effort> {
  verb
    .get_one::<String>("effort")
    .and_then(|name| Effort::parse(name))
}

#[cfg(test)]
#[path = "tests/cli_test.rs"]
mod tests;
