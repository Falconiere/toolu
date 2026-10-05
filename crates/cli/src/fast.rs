//! The hook fast path (#442, #410 budget): the argv forms `hooks.json` and the
//! launcher use, recognised before the clap tree is built. Only a leaf plugin's
//! own name starts a plugin hook line, and each flag appears at most once.
//! Anything else, a malformed hook line included, falls through to clap, whose
//! `hook` command (`command`) mirrors this grammar and reports the error.

use clap::{Arg, ArgMatches, Command};
use toolu_protocol::launcher::is_name;

use crate::registry::is_hook_owner;

/// A command line the fast path answers without the clap tree.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Fast {
  /// `toolu --hook-protocol`: the native signature the launcher checks.
  HookProtocol,
  /// `toolu [<plugin>] hook <name> …`.
  Hook(HookRequest),
}

/// One hook run.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct HookRequest {
  /// The plugin namespace; `toolu` when the argv starts with `hook`.
  pub(crate) plugin: String,
  /// The hook name.
  pub(crate) name: String,
  /// The host event (`--event`); absent counts as enforcing.
  pub(crate) event: Option<String>,
  /// The calling plugin's root (`--plugin-root`); absent skips the skew prelude.
  pub(crate) plugin_root: Option<String>,
}

/// The fast-path form `words` (argv after the program name) is, if any.
pub(crate) fn parse(words: &[String]) -> Option<Fast> {
  let strs: Vec<&str> = words.iter().map(String::as_str).collect();
  match strs.as_slice() {
    ["--hook-protocol"] => Some(Fast::HookProtocol),
    ["hook", name, rest @ ..] => hook("toolu", name, rest),
    [plugin, "hook", name, rest @ ..] if is_hook_owner(plugin) => hook(plugin, name, rest),
    _ => None,
  }
}

fn hook(plugin: &str, name: &str, flags: &[&str]) -> Option<Fast> {
  if !is_name(name) {
    return None;
  }
  let mut request = HookRequest {
    plugin: plugin.to_owned(),
    name: name.to_owned(),
    event: None,
    plugin_root: None,
  };
  let mut rest = flags.iter();
  while let Some(flag) = rest.next() {
    let value = rest.next()?;
    let slot = match *flag {
      "--event" => &mut request.event,
      "--plugin-root" => &mut request.plugin_root,
      _ => return None,
    };
    if slot.is_some() {
      return None;
    }
    *slot = Some((*value).to_owned());
  }
  Some(Fast::Hook(request))
}

/// The clap mirror of the hook grammar: `hook <NAME> [--event E] [--plugin-root D]`.
pub(crate) fn command() -> Command {
  Command::new("hook")
    .about("Run a hook entry; hooks.json calls it (internal)")
    .arg(
      Arg::new("name")
        .value_name("NAME")
        .required(true)
        .value_parser(hook_name)
        .help("The hook entry, as in the plugin's hooks/dist/<NAME>.js"),
    )
    .arg(
      Arg::new("event")
        .long("event")
        .value_name("EVENT")
        .help("The host event it is registered under; absent counts as enforcing"),
    )
    .arg(
      Arg::new("plugin-root")
        .long("plugin-root")
        .value_name("DIR")
        .help("The calling plugin's root, for the version-skew check"),
    )
}

fn hook_name(text: &str) -> Result<String, String> {
  if is_name(text) {
    Ok(text.to_owned())
  } else {
    Err("a hook name matches ^[a-z0-9]+(-[a-z0-9]+)*$".to_owned())
  }
}

/// The request clap parsed for `plugin`'s `hook` command.
pub(crate) fn request(plugin: &str, matches: &ArgMatches) -> HookRequest {
  let value = |id: &str| matches.get_one::<String>(id).cloned();
  HookRequest {
    plugin: plugin.to_owned(),
    name: value("name").unwrap_or_default(),
    event: value("event"),
    plugin_root: value("plugin-root"),
  }
}

#[cfg(test)]
#[path = "tests/fast_test.rs"]
mod tests;
