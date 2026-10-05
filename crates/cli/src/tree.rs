//! The full clap command tree of `toolu` (#442): the root with its global flags
//! and every namespace of the registry. Hooks never build it (`crate::fast`).

use std::fmt::Write as _;
use std::path::PathBuf;

use clap::builder::PossibleValuesParser;
use clap::{Arg, ArgAction, Command, value_parser};
use toolu_protocol::exit::Exit;
use toolu_protocol::host::Host;

use crate::registry::{self, NAMESPACES};
use crate::{VERSION, fast};

/// One line for `toolu --help` and the command tree.
pub(crate) const ABOUT: &str =
  "The one binary behind every toolu plugin's hooks, skills and commands";

/// The whole tree.
pub(crate) fn command() -> Command {
  let root = Command::new("toolu")
    .version(VERSION)
    .about(ABOUT)
    .after_long_help(contract())
    .arg_required_else_help(true)
    .args(global_flags())
    .arg(
      Arg::new("hook-protocol")
        .long("hook-protocol")
        .action(ArgAction::SetTrue)
        .hide(true)
        .help("Print the hook-interface version the launcher checks"),
    );
  NAMESPACES.iter().fold(root, |root, namespace| {
    root.subcommand(decorated((namespace.command)(), namespace.owner))
  })
}

fn global_flags() -> [Arg; 4] {
  let hosts: Vec<&str> = Host::ALL.iter().map(|host| host.name()).collect();
  [
    Arg::new("json")
      .long("json")
      .global(true)
      .action(ArgAction::SetTrue)
      .help("Print exactly one JSON document on stdout"),
    Arg::new("quiet")
      .short('q')
      .long("quiet")
      .global(true)
      .action(ArgAction::SetTrue)
      .help("Drop the diagnostics of a successful run"),
    Arg::new("host")
      .long("host")
      .global(true)
      .value_name("HOST")
      .value_parser(PossibleValuesParser::new(hosts))
      .help("The host to act for, instead of the detected one"),
    Arg::new("config-dir")
      .long("config-dir")
      .global(true)
      .value_name("DIR")
      .value_parser(value_parser!(PathBuf))
      .help("Read the toolu config from DIR instead of the host's default"),
  ]
}

/// A leaf plugin's namespace gains the hidden `hook` verb its `hooks.json`
/// entries call, and its plugin name as an alias when the two differ.
fn decorated(command: Command, owner: &'static str) -> Command {
  if !registry::is_leaf_plugin(owner) {
    return command;
  }
  let command = command.subcommand(fast::command().hide(true));
  if command.get_name() == owner {
    command
  } else {
    command.visible_alias(owner)
  }
}

/// The streams and exit codes, shown by `toolu --help`.
fn contract() -> String {
  let mut text = String::from(
    "Output: data on stdout, diagnostics on stderr. With --json, stdout is exactly one \
     JSON document (hooks speak their host's protocol instead).\n\nExit codes:",
  );
  for exit in Exit::ALL {
    // Writing to a String cannot fail.
    write!(
      text,
      "\n  {:<3} {:<12} {}",
      exit.code(),
      exit.name(),
      exit.meaning()
    )
    .ok();
  }
  text
}

#[cfg(test)]
#[path = "tests/tree_test.rs"]
mod tests;
