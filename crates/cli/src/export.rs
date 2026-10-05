//! The command tree as a `toolu.commands/v1` document (#442), the input of the
//! docs generator and #444's drift gate. It carries no version, so a release
//! never makes `docs/cli/commands.json` stale; `hookProtocol` changes only when
//! a documented verb breaks (#411).

use clap::builder::StyledStr;
use clap::{Arg, Command};
use serde_json::{Value, json};
use toolu_protocol::HOOK_PROTOCOL;
use toolu_protocol::exit::Exit;
use toolu_runtime::namespace::PLACEHOLDER;

use crate::registry::{self, BUILTIN};

/// The document's `schema` value.
pub(crate) const SCHEMA_ID: &str = "toolu.commands/v1";

/// The document for `root`, the whole tree.
pub(crate) fn tree(root: &Command) -> Value {
  let mut root = root.clone();
  root.build();
  let exit_codes: Vec<Value> = Exit::ALL
    .iter()
    .map(|exit| json!({ "code": exit.code(), "name": exit.name(), "meaning": exit.meaning() }))
    .collect();
  let commands: Vec<Value> = subcommands(&root)
    .map(|command| node(command, &[], owner_of(command.get_name())))
    .collect();
  json!({
    "schema": SCHEMA_ID,
    "name": root.get_name(),
    "about": text(root.get_about()),
    "hookProtocol": HOOK_PROTOCOL,
    "exitCodes": exit_codes,
    "flags": flags(&root, true),
    "commands": commands,
  })
}

fn owner_of(name: &str) -> &'static str {
  registry::find(name).map_or(BUILTIN, |namespace| namespace.owner)
}

/// Subcommands, without the `help` command clap adds while building.
fn subcommands(command: &Command) -> impl Iterator<Item = &Command> {
  command
    .get_subcommands()
    .filter(|sub| sub.get_name() != "help")
}

fn node(command: &Command, parent: &[String], owner: &str) -> Value {
  let mut path = parent.to_vec();
  path.push(command.get_name().to_owned());
  let children: Vec<Value> = subcommands(command)
    .map(|sub| node(sub, &path, owner))
    .collect();
  let aliases: Vec<&str> = command.get_visible_aliases().collect();
  let args: Vec<Value> = command
    .get_positionals()
    .map(|arg| {
      json!({
        "name": value_name(arg).unwrap_or_else(|| arg.get_id().as_str().to_uppercase()),
        "required": arg.is_required_set(),
        "multiple": arg.get_num_args().is_some_and(|range| range.max_values() > 1),
        "help": text(arg.get_help()),
      })
    })
    .collect();
  json!({
    "name": command.get_name(),
    "path": path,
    "about": text(command.get_about()),
    "longAbout": command.get_long_about().map(ToString::to_string),
    "aliases": aliases,
    "owner": owner,
    "hidden": command.is_hide_set(),
    "placeholder": command.get_name() == PLACEHOLDER,
    "flags": flags(command, false),
    "args": args,
    "commands": children,
  })
}

/// Named arguments, without the implicit `--help`; global ones only at the root.
fn flags(command: &Command, root: bool) -> Vec<Value> {
  command
    .get_arguments()
    .filter(|arg| !arg.is_positional() && arg.get_id() != "help")
    .filter(|arg| root || !arg.is_global_set())
    .map(flag)
    .collect()
}

fn flag(arg: &Arg) -> Value {
  let takes_value = arg.get_action().takes_values();
  let values: Vec<String> = if takes_value {
    arg
      .get_possible_values()
      .iter()
      .filter(|value| !value.is_hide_set())
      .map(|value| value.get_name().to_owned())
      .collect()
  } else {
    Vec::new()
  };
  json!({
    "long": arg.get_long().unwrap_or_default(),
    "short": arg.get_short().map(String::from),
    "valueName": if takes_value { value_name(arg) } else { None },
    "takesValue": takes_value,
    "required": arg.is_required_set(),
    "global": arg.is_global_set(),
    "hidden": arg.is_hide_set(),
    "possibleValues": values,
    "help": text(arg.get_help()),
  })
}

fn value_name(arg: &Arg) -> Option<String> {
  arg
    .get_value_names()
    .and_then(|names| names.first())
    .map(ToString::to_string)
}

fn text(styled: Option<&StyledStr>) -> String {
  styled.map(ToString::to_string).unwrap_or_default()
}

#[cfg(test)]
#[path = "tests/export_test.rs"]
mod tests;
