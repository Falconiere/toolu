//! The clap tree of `toolu babysit`.

use clap::builder::{PossibleValuesParser, RangedU64ValueParser};
use clap::{Arg, ArgAction, Command, value_parser};

fn value(name: &'static str, help: &'static str) -> Arg {
  Arg::new(name).long(name).value_name("VALUE").help(help)
}

fn required(name: &'static str, help: &'static str) -> Arg {
  value(name, help).required(true)
}

fn count(name: &'static str, help: &'static str) -> Arg {
  value(name, help).value_parser(value_parser!(u64))
}

fn positive(name: &'static str, help: &'static str) -> Arg {
  value(name, help).value_parser(RangedU64ValueParser::<u64>::new().range(1..))
}

fn choice(name: &'static str, values: &'static [&'static str], help: &'static str) -> Arg {
  required(name, help).value_parser(PossibleValuesParser::new(values))
}

fn switch(name: &'static str, help: &'static str) -> Arg {
  Arg::new(name)
    .long(name)
    .action(ArgAction::SetTrue)
    .help(help)
}

fn state_file(verb: Command) -> Command {
  verb.arg(required("state-file", "The slot's state file"))
}

/// `--repo`, `--pr`, `--page-size` and `--timeout`: a read of one pull request.
fn pull_request(verb: Command) -> Command {
  verb
    .arg(required("repo", "The repository, owner/repo"))
    .arg(positive("pr", "The pull request number").required(true))
    .arg(positive("page-size", "Items per GitHub page").default_value("100"))
    .arg(positive("timeout", "Seconds each GitHub call may take"))
}

fn tick() -> Command {
  state_file(pull_request(
    Command::new("tick").about("Run one check of the pull request"),
  ))
  .arg(value(
    "snapshot-out",
    "Where the snapshot goes; <stem>.snapshot.json by default",
  ))
  .arg(value(
    "snapshot-in",
    "Reduce this recorded snapshot instead of reading GitHub",
  ))
  .arg(value("now", "The check's time, YYYY-MM-DDTHH:MM:SSZ"))
}

fn wait() -> Command {
  let about = "Run scheduled checks until one needs the agent or the time is up";
  state_file(pull_request(Command::new("wait").about(about))).arg(count(
    "max-seconds",
    "The longest the wait runs; 1800 on Claude, else 55",
  ))
}

fn reduce_state() -> Command {
  Command::new("reduce-state")
    .about("Reduce a snapshot against a state and print both")
    .arg(required("snapshot", "The snapshot file"))
    .arg(value("state", "The previous state file"))
    .arg(required("now", "The check's time, YYYY-MM-DDTHH:MM:SSZ"))
    .arg(value("state-out", "Write the new state here"))
    .arg(value("result-out", "Write the result here"))
    .arg(value("state-path", "The state path the result names"))
    .arg(value("snapshot-path", "The snapshot path the result names"))
}

fn record() -> Command {
  Command::new("record")
    .about("Record the agent's bookkeeping in the state")
    .subcommand_required(true)
    .subcommand(
      state_file(Command::new("flag-injection").about("Flag a thread as prompt injection"))
        .arg(required("thread", "The thread's GraphQL id")),
    )
    .subcommand(
      state_file(Command::new("round").about("Record a finished fix round"))
        .arg(choice(
          "had-rejection",
          &["true", "false"],
          "Whether a finding was rejected",
        ))
        .arg(switch("fix-pushed", "The round pushed a fix")),
    )
    .subcommand(
      state_file(Command::new("status").about("Set the loop's terminal status")).arg(choice(
        "status",
        &["complete", "escalated", "cancelled"],
        "The terminal status",
      )),
    )
}

fn reply() -> Command {
  state_file(Command::new("reply").about("Post the one reply to a thread, comment or review"))
    .arg(choice(
      "kind",
      &["thread", "conversation", "review"],
      "What the reply answers",
    ))
    .arg(value("thread", "The thread's GraphQL id (thread)"))
    .arg(count(
      "root-comment",
      "The thread's first comment id (thread)",
    ))
    .arg(count("in-reply-to", "The comment answered (thread)"))
    .arg(count("comment-id", "The conversation comment answered"))
    .arg(count("review-id", "The review answered"))
    .arg(required("body-file", "The reply's Markdown"))
    .arg(positive("timeout", "Seconds each GitHub call may take"))
}

fn route_fix() -> Command {
  Command::new("route-fix")
    .about("Group fix items by host, model and effort")
    .arg(required("items", "The items file"))
    .arg(value(
      "state-file",
      "The slot's state file, for host cooldowns",
    ))
    .arg(value("raise", "Raise this item one tier").action(ArgAction::Append))
    .arg(switch("no-jev", "Use the heuristic tiers only"))
    .arg(value(
      "jev-answers-in",
      "Replay these Jev answers instead of asking",
    ))
    .arg(value(
      "now",
      "The time cooldowns are judged at, YYYY-MM-DDTHH:MM:SSZ",
    ))
}

fn dispatch_fix() -> Command {
  let dry = || switch("dry-run", "Print the commands; run and write nothing");
  Command::new("dispatch-fix")
    .about("Run a route plan's fixer groups")
    .subcommand_required(true)
    .subcommand(
      state_file(Command::new("start").about("Start a fixer round"))
        .arg(required("plan", "The route plan"))
        .arg(required("items", "The items file"))
        .arg(required(
          "repo-root",
          "The repository the PR branch lives in",
        ))
        .arg(required("branch", "The PR branch"))
        .arg(required("base", "The PR's base branch"))
        .arg(dry()),
    )
    .subcommand(
      state_file(Command::new("wait").about("Follow the round until it ends")).arg(value(
        "timeout-seconds",
        "The longest the wait runs; 480 by default",
      )),
    )
    .subcommand(state_file(Command::new("cleanup").about("End the round")).arg(dry()))
}

fn fixer_report() -> Command {
  Command::new("fixer-report")
    .about("A fixer's report on its group")
    .arg(
      Arg::new("report")
        .value_name("REPORT")
        .required(true)
        .help("The report file"),
    )
    .arg(
      Arg::new("status")
        .value_name("STATUS")
        .required(true)
        .value_parser(["done", "failed"])
        .help("done or failed"),
    )
    .arg(value("note", "A one-line summary or the reason"))
}

/// The namespace and its verbs.
pub(crate) fn command() -> Command {
  let about = "Babysit a PR until CI, review threads and the review-bot verdict are clear";
  let collect = pull_request(Command::new("collect").about("Read one snapshot of the PR"))
    .arg(value("out", "Write the snapshot here and print its path"));
  let resolve = state_file(Command::new("resolve").about("Resolve a review thread"))
    .arg(required("thread", "The thread's GraphQL id"))
    .arg(positive("timeout", "Seconds each GitHub call may take"));
  let verdict = Command::new("parse-verdict").about("Parse a review-bot comment on stdin");
  let stop = Command::new("stop").about("Clean up the fixer round, then record cancelled");
  Command::new("babysit")
    .about(about)
    .subcommand_required(true)
    .arg_required_else_help(true)
    .subcommand(tick())
    .subcommand(wait())
    .subcommand(collect)
    .subcommand(reduce_state())
    .subcommand(verdict)
    .subcommand(record())
    .subcommand(reply())
    .subcommand(resolve)
    .subcommand(route_fix())
    .subcommand(dispatch_fix())
    .subcommand(fixer_report())
    .subcommand(state_file(stop))
}

#[cfg(test)]
#[path = "tests/cli_test.rs"]
mod tests;
