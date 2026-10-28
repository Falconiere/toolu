//! The `toolu git-better` namespace: `check` runs the rule's checks on a message
//! or branch name before the agent commits or creates it, and `setup` makes
//! amend and rebase carry the provenance notes.

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
    .subcommand(
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
        ),
    )
    .subcommand(Command::new("setup").about(
      "Make amend and rebase carry git-better notes: add refs/notes/git-better to this repository's notes.rewriteRef",
    ))
}

/// Whether the verb reads its message from stdin.
pub(crate) fn needs_stdin(matches: &ArgMatches) -> bool {
  matches.subcommand().is_some_and(|(verb, args)| {
    verb == "check" && args.get_one::<String>("message-file").map(String::as_str) == Some("-")
  })
}

/// The process environment, with `--config-dir` as `TOOLU_CONFIG_DIR`.
pub(crate) fn env_of(ctx: &Ctx) -> Env {
  let env = Env::process();
  match &ctx.config_dir {
    Some(dir) => env.with("TOOLU_CONFIG_DIR", &dir.to_string_lossy()),
    None => env,
  }
}

/// Run a `toolu git-better` verb.
pub(crate) fn run(matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  match matches.subcommand() {
    Some(("check", args)) => crate::check::check(args, ctx),
    Some(("setup", _)) => crate::setup::setup(ctx),
    _ => Outcome::failed(Exit::Usage, "a verb is required".to_owned()),
  }
}

#[cfg(test)]
#[path = "tests/cli_test.rs"]
mod tests;
