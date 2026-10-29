//! `toolu completions`: a bash, zsh, or fish script for the command tree.

use clap::builder::PossibleValuesParser;
use clap::{Arg, ArgMatches, Command};
use clap_complete::generate;
use clap_complete::shells::{Bash, Fish, Zsh};
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::{Ctx, Outcome};

/// The shells `toolu completions` prints a script for.
const SHELLS: [&str; 3] = ["bash", "zsh", "fish"];

/// The `completions` command.
pub(crate) fn command() -> Command {
  Command::new("completions")
    .about("Print a bash, zsh, or fish completion script")
    .arg(
      Arg::new("shell")
        .value_name("SHELL")
        .required(true)
        .value_parser(PossibleValuesParser::new(SHELLS))
        .help("The shell the script is for"),
    )
}

/// Print the completion script for the shell named on `matches`.
pub(crate) fn run(matches: &ArgMatches, ctx: &Ctx, tree: &dyn Fn() -> Command) -> Outcome {
  if ctx.json {
    return Outcome::failed(
      Exit::Usage,
      "toolu completions prints a shell script, not a JSON document".to_owned(),
    );
  }
  let Some(shell) = matches.get_one::<String>("shell") else {
    return Outcome::failed(
      Exit::Usage,
      "toolu completions: a shell is required".to_owned(),
    );
  };
  let mut bytes = Vec::new();
  if !write(shell, &mut visible(&tree()), &mut bytes) {
    return Outcome::failed(
      Exit::Usage,
      format!("toolu completions: unknown shell {shell}"),
    );
  }
  let Ok(text) = String::from_utf8(bytes) else {
    return Outcome::failed(
      Exit::Failure,
      "toolu completions: the script was not UTF-8".to_owned(),
    );
  };
  Outcome::data(text.trim_end().to_owned())
}

/// A copy of `command` without hidden subcommands. `clap_complete` prints those,
/// and plugin `hook` verbs are hidden.
fn visible(command: &Command) -> Command {
  let mut built = Command::new(command.get_name().to_owned());
  if let Some(about) = command.get_about() {
    built = built.about(about.to_string());
  }
  if let Some(about) = command.get_long_about() {
    built = built.long_about(about.to_string());
  }
  if let Some(version) = command.get_version() {
    built = built.version(version.to_owned());
  }
  for alias in command.get_visible_aliases() {
    built = built.visible_alias(alias.to_owned());
  }
  for arg in command.get_arguments() {
    built = built.arg(arg.clone());
  }
  for sub in command.get_subcommands().filter(|sub| !sub.is_hide_set()) {
    built = built.subcommand(visible(sub));
  }
  built
}

/// Write the completion script for `shell` into `bytes`. `false` when `shell` is not one of [`SHELLS`].
fn write(shell: &str, root: &mut Command, bytes: &mut Vec<u8>) -> bool {
  match shell {
    "bash" => generate(Bash, root, "toolu", bytes),
    "zsh" => generate(Zsh, root, "toolu", bytes),
    "fish" => generate(Fish, root, "toolu", bytes),
    _ => return false,
  }
  true
}

#[cfg(test)]
#[path = "tests/completions_test.rs"]
mod tests;
