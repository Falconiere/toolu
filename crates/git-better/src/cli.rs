//! The `toolu git-better` namespace: `check` runs the rule's checks on a message
//! or branch name before the agent commits or creates it, and `setup` makes
//! amend and rebase carry the provenance notes.

use std::path::PathBuf;

use clap::{Arg, ArgAction, ArgMatches, Command};
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::env::Env;

/// The namespace and its verbs.
pub(crate) fn command() -> Command {
  Command::new("git-better")
    .about("Semantic branches and commits that agents can read")
    .subcommand_required(true)
    .arg_required_else_help(true)
    .subcommand(check_command())
    .subcommand(
      Command::new("log")
        .about("Each commit of a range with its intent, areas, risk and refs; the branch's commits by default")
        .arg(Arg::new("range").value_name("RANGE").help("A revision range, such as main..HEAD"))
        .arg(limit()),
    )
    .subcommand(
      Command::new("context")
        .about("What the current branch does against its base: commits, intents, areas, risk, refs and the working tree")
        .arg(limit()),
    )
    .subcommand(
      Command::new("pr")
        .about("Draft a PR title and body, or with --squash a squash message, from the branch's commits")
        .arg(
          Arg::new("squash")
            .long("squash")
            .action(ArgAction::SetTrue)
            .help("Print the squash commit message, its trailers merged"),
        )
        .arg(limit()),
    )
    .subcommand(Command::new("setup").about(
      "Make amend and rebase carry git-better notes: add refs/notes/git-better to this repository's notes.rewriteRef",
    ))
}

/// `check` and its message and branch arguments.
fn check_command() -> Command {
  Command::new("check")
    .about(
      "Check a commit message or branch name against the semantic format; exit 1 when it fails",
    )
    .arg(
      Arg::new("message")
        .short('m')
        .long("message")
        .value_name("TEXT")
        .action(ArgAction::Append)
        .help("A message paragraph, as `git commit -m` takes it; repeat for more"),
    )
    .arg(
      Arg::new("message-file")
        .short('F')
        .long("message-file")
        .value_name("FILE")
        .conflicts_with("message")
        .help("Read the message from FILE, or from stdin with `-`"),
    )
    .arg(
      Arg::new("branch")
        .long("branch")
        .value_name("NAME")
        .help("A new branch name to check"),
    )
}

/// `--limit N`: how many commits to read, 1 to 200.
fn limit() -> Arg {
  Arg::new("limit")
    .long("limit")
    .value_name("N")
    .value_parser(clap::value_parser!(u64).range(1..=200))
    .help("Read at most N commits, newest first (default 20)")
}

/// Whether the verb reads its message from stdin.
pub(crate) fn needs_stdin(matches: &ArgMatches) -> bool {
  matches.subcommand().is_some_and(|(verb, args)| {
    verb == "check" && args.get_one::<String>("message-file").map(String::as_str) == Some("-")
  })
}

/// Where a verb runs: its environment and directory.
pub(crate) struct Place {
  /// The environment, `TOOLU_CONFIG_DIR` set from `--config-dir`.
  pub(crate) env: Env,
  /// The directory whose repository the verb reads.
  pub(crate) dir: PathBuf,
}

impl Place {
  /// This process's place.
  fn of(ctx: &Ctx) -> Place {
    let env = Env::process();
    let env = match &ctx.config_dir {
      Some(dir) => env.with("TOOLU_CONFIG_DIR", &dir.to_string_lossy()),
      None => env,
    };
    Place {
      env,
      dir: std::env::current_dir().unwrap_or_default(),
    }
  }
}

/// Run a `toolu git-better` verb.
pub(crate) fn run(matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  let place = Place::of(ctx);
  match matches.subcommand() {
    Some(("check", args)) => crate::check::check(args, ctx, &place),
    Some(("log", args)) => crate::read::log(args, ctx, &place),
    Some(("context", args)) => crate::read::context(args, ctx, &place),
    Some(("pr", args)) => crate::read::pr(args, ctx, &place),
    Some(("setup", _)) => crate::setup::setup(ctx, &place),
    _ => Outcome::failed(Exit::Usage, "a verb is required".to_owned()),
  }
}

#[cfg(test)]
#[path = "tests/cli_test.rs"]
mod tests;
