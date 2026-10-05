//! `toolu commands` (#442): every command as a list, the tree as one JSON
//! document (`--json`), or the JSON Schema of every `--json` document
//! (`--schema`).

use std::fmt::Write as _;

use clap::{Arg, ArgAction, ArgMatches, Command};
use toolu_runtime::cli::{Ctx, Outcome};

use crate::export;

/// The JSON Schema (draft 2020-12) of `toolu.commands/v1` and of every other
/// `--json` document, in its `$defs`.
pub(crate) const SCHEMA: &str = include_str!("commands.schema.json");

/// The `commands` command.
pub(crate) fn command() -> Command {
  Command::new("commands")
    .about("List every toolu command; --json exports the tree for tools and docs")
    .arg(
      Arg::new("schema")
        .long("schema")
        .action(ArgAction::SetTrue)
        .help("Print the JSON Schema of every --json document instead"),
    )
}

/// Run `toolu commands` against the tree `tree` builds.
pub(crate) fn run(matches: &ArgMatches, ctx: &Ctx, tree: &dyn Fn() -> Command) -> Outcome {
  if matches.get_flag("schema") {
    return Outcome::data(SCHEMA.trim_end().to_owned());
  }
  let root = tree();
  if ctx.json {
    return Outcome::data(format!("{:#}", export::tree(&root)));
  }
  Outcome::data(listing(&root))
}

/// One line per visible command: `toolu <path>`, padded, then its about.
fn listing(root: &Command) -> String {
  let mut rows = Vec::new();
  collect(root, root.get_name(), &mut rows);
  let width = rows.iter().map(|(path, _)| path.len()).max().unwrap_or(0);
  let mut text = String::new();
  for (path, about) in rows {
    // Writing to a String cannot fail.
    writeln!(text, "{path:<width$}  {about}").ok();
  }
  text.trim_end().to_owned()
}

fn collect(command: &Command, path: &str, rows: &mut Vec<(String, String)>) {
  for sub in command.get_subcommands().filter(|sub| !sub.is_hide_set()) {
    let sub_path = format!("{path} {}", sub.get_name());
    let about = sub.get_about().map(ToString::to_string).unwrap_or_default();
    rows.push((sub_path.clone(), about));
    collect(sub, &sub_path, rows);
  }
}

#[cfg(test)]
#[path = "tests/commands_test.rs"]
mod tests;
