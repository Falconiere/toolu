//! Every top-level command of `toolu` and the plugin that owns it (#442). Plugin
//! crates contribute their namespaces here; the rule crates come through the hub
//! (`toolu_hub`), because only the hub may link them (#460).

use clap::{ArgMatches, Command};
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::namespace::Planned;

use crate::{commands, fast};

/// The owner of the commands `crates/cli` provides itself.
pub(crate) const BUILTIN: &str = "toolu-cli";

/// A plugin verb's entry point: `run(&ArgMatches, &Ctx)`.
pub(crate) type Verb = fn(&ArgMatches, &Ctx) -> Outcome;

/// What running a top-level command does.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Action {
  /// A namespace's `run`.
  Verb(Verb),
  /// `toolu hook`: #412's hook runner, which needs the process context.
  Hook,
  /// `toolu commands`: the command tree, which needs the tree itself.
  Commands,
}

/// One top-level command.
#[derive(Debug)]
pub(crate) struct Namespace {
  /// Its name, `command().get_name()`, kept here so the hook fast path can read
  /// it without building a clap command.
  pub(crate) name: &'static str,
  /// The plugin directory that owns it, or `BUILTIN`.
  pub(crate) owner: &'static str,
  /// Its clap command.
  pub(crate) command: fn() -> Command,
  /// What running it does.
  pub(crate) action: Action,
}

const fn verb(
  name: &'static str,
  owner: &'static str,
  command: fn() -> Command,
  run: Verb,
) -> Namespace {
  Namespace {
    name,
    owner,
    command,
    action: Action::Verb(run),
  }
}

/// `toolu plugins`: the installer, still the npm package `@toolu/plugins` until #438.
const PLUGINS: Planned = Planned {
  name: "plugins",
  about: "Install, list, remove and update toolu plugins on every host",
  verbs: &["install", "list", "remove", "update"],
  issues: &[438],
};

fn plugins_command() -> Command {
  PLUGINS.command().after_help(
    "Until #438 ports it, install plugins with `npx @toolu/plugins install`; \
     its guide is docs/cli/installer.md.",
  )
}

fn plugins_run(_matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  PLUGINS.run(ctx)
}

/// The top-level commands, in `toolu --help` order.
pub(crate) const NAMESPACES: &[Namespace] = &[
  Namespace {
    name: "hook",
    owner: toolu_hub::PLUGIN,
    command: fast::command,
    action: Action::Hook,
  },
  verb(
    "ledger",
    toolu_hub::PLUGIN,
    toolu_hub::ledger::command,
    toolu_hub::ledger::run,
  ),
  verb(
    "debug",
    toolu_hub::PLUGIN,
    toolu_hub::debug::command,
    toolu_hub::debug::run,
  ),
  verb(
    "setup",
    toolu_hub::PLUGIN,
    toolu_hub::setup::command,
    toolu_hub::setup::run,
  ),
  verb(
    "doctor",
    toolu_hub::PLUGIN,
    toolu_hub::doctor::command,
    toolu_hub::doctor::run,
  ),
  verb(
    "config",
    toolu_hub::PLUGIN,
    toolu_hub::config::command,
    toolu_hub::config::run,
  ),
  verb(
    "status",
    toolu_hub::PLUGIN,
    toolu_hub::status::command,
    toolu_hub::status::run,
  ),
  verb(
    "serve",
    toolu_hub::PLUGIN,
    toolu_hub::serve::command,
    toolu_hub::serve::run,
  ),
  verb(
    "ts-quality",
    toolu_hub::ts_quality::PLUGIN,
    toolu_hub::ts_quality::command,
    toolu_hub::ts_quality::run,
  ),
  verb(
    "python-quality",
    toolu_hub::python_quality::PLUGIN,
    toolu_hub::python_quality::command,
    toolu_hub::python_quality::run,
  ),
  verb(
    "rust-quality",
    toolu_hub::rust_quality::PLUGIN,
    toolu_hub::rust_quality::command,
    toolu_hub::rust_quality::run,
  ),
  verb(
    "ast-grep",
    toolu_hub::ast_grep::PLUGIN,
    toolu_hub::ast_grep::command,
    toolu_hub::ast_grep::run,
  ),
  verb(
    "brainstorm",
    toolu_brainstorm::PLUGIN,
    toolu_brainstorm::command,
    toolu_brainstorm::run,
  ),
  verb(
    "delivery-flow",
    toolu_delivery_flow::PLUGIN,
    toolu_delivery_flow::command,
    toolu_delivery_flow::run,
  ),
  verb(
    "review",
    toolu_review::PLUGIN,
    toolu_review::command,
    toolu_review::run,
  ),
  verb("jev", toolu_jev::PLUGIN, toolu_jev::command, toolu_jev::run),
  verb(
    "statusline",
    toolu_statusline::PLUGIN,
    toolu_statusline::command,
    toolu_statusline::run,
  ),
  verb(
    "babysit",
    toolu_pr_babysit::PLUGIN,
    toolu_pr_babysit::command,
    toolu_pr_babysit::run,
  ),
  verb(
    "epic",
    toolu_epic_orchestrator::PLUGIN,
    toolu_epic_orchestrator::command,
    toolu_epic_orchestrator::run,
  ),
  Namespace {
    name: "commands",
    owner: BUILTIN,
    command: commands::command,
    action: Action::Commands,
  },
  verb("plugins", BUILTIN, plugins_command, plugins_run),
];

/// The namespace whose command is `name`.
pub(crate) fn find(name: &str) -> Option<&'static Namespace> {
  NAMESPACES.iter().find(|namespace| namespace.name == name)
}

/// Whether `owner` is a plugin other than toolu: its namespace carries a hidden
/// `hook` verb and, when the names differ, the plugin name as an alias.
pub(crate) fn is_leaf_plugin(owner: &str) -> bool {
  owner != toolu_hub::PLUGIN && owner != BUILTIN
}

/// Whether `word` may start a plugin hook line on the fast path
/// (`toolu <plugin> hook <name>`, the `hooks.json` form): a leaf plugin's own
/// name, or a name this binary does not know, a newer plugin's, which the skew
/// prelude (#411) answers with the upgrade advice. A built-in command or a
/// namespace spelled unlike its plugin (`review`) goes through clap instead.
/// Reads the static names only: no clap command is built.
pub(crate) fn starts_hook_line(word: &str) -> bool {
  let owner = NAMESPACES.iter().any(|namespace| namespace.owner == word);
  let known = NAMESPACES.iter().any(|namespace| namespace.name == word);
  is_leaf_plugin(word) && (owner || !known)
}

#[cfg(test)]
#[path = "tests/registry_test.rs"]
mod tests;
