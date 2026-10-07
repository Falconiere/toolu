//! `toolu config`: read, change and validate toolu.config.json (#445).

use std::path::PathBuf;

use clap::{Arg, ArgAction, ArgMatches, Command};
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::host::roots::Roots;

mod edit;
mod show;

/// Read, change and validate toolu.config.json.
pub fn command() -> Command {
  Command::new("config")
    .about("Read, change and validate toolu.config.json")
    .subcommand(
      Command::new("get")
        .about("Print one key or the redacted merged config")
        .arg(Arg::new("key").help("Dotted key")),
    )
    .subcommand(
      Command::new("set")
        .about("Set one key in the user config, or the project config with --project")
        .arg(Arg::new("key").required(true).help("Dotted key"))
        .arg(
          Arg::new("value")
            .required(true)
            .help("JSON value, or a string when it is not JSON"),
        )
        .arg(
          Arg::new("project")
            .long("project")
            .action(ArgAction::SetTrue)
            .help("Write the project file"),
        ),
    )
    .subcommand(Command::new("validate").about("Validate the merged config and epic settings"))
}

/// Run `get`, `set` or `validate`.
pub fn run(matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  match matches.subcommand() {
    Some(("get", sub)) => show::get(ctx, sub.get_one::<String>("key").map(String::as_str)),
    Some(("set", sub)) => edit::set(
      ctx,
      sub.get_one::<String>("key").map_or("", String::as_str),
      sub.get_one::<String>("value").map_or("", String::as_str),
      sub.get_flag("project"),
    ),
    Some(("validate", _)) => show::validate(ctx),
    _ => Outcome::failed(Exit::Usage, "toolu config: a verb is required".to_owned()),
  }
}

/// Roots for `ctx`, with `--config-dir` overlaid, and the working directory.
pub(super) fn place(ctx: &Ctx) -> (Roots, PathBuf) {
  crate::overlaid_roots(ctx)
}

#[cfg(test)]
#[path = "tests/config_test.rs"]
mod tests;
