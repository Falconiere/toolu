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
  /// The plugin directory that owns it, or `BUILTIN`.
  pub(crate) owner: &'static str,
  /// Its clap command.
  pub(crate) command: fn() -> Command,
  /// What running it does.
  pub(crate) action: Action,
}

const fn verb(owner: &'static str, command: fn() -> Command, run: Verb) -> Namespace {
  Namespace {
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
    owner: toolu_hub::PLUGIN,
    command: fast::command,
    action: Action::Hook,
  },
  verb(
    toolu_hub::PLUGIN,
    toolu_hub::ledger::command,
    toolu_hub::ledger::run,
  ),
  verb(
    toolu_hub::PLUGIN,
    toolu_hub::debug::command,
    toolu_hub::debug::run,
  ),
  verb(
    toolu_hub::PLUGIN,
    toolu_hub::setup::command,
    toolu_hub::setup::run,
  ),
  verb(
    toolu_hub::PLUGIN,
    toolu_hub::doctor::command,
    toolu_hub::doctor::run,
  ),
  verb(
    toolu_hub::PLUGIN,
    toolu_hub::config::command,
    toolu_hub::config::run,
  ),
  verb(
    toolu_hub::PLUGIN,
    toolu_hub::status::command,
    toolu_hub::status::run,
  ),
  verb(
    toolu_hub::PLUGIN,
    toolu_hub::serve::command,
    toolu_hub::serve::run,
  ),
  verb(
    toolu_hub::ts_quality::PLUGIN,
    toolu_hub::ts_quality::command,
    toolu_hub::ts_quality::run,
  ),
  verb(
    toolu_hub::python_quality::PLUGIN,
    toolu_hub::python_quality::command,
    toolu_hub::python_quality::run,
  ),
  verb(
    toolu_hub::rust_quality::PLUGIN,
    toolu_hub::rust_quality::command,
    toolu_hub::rust_quality::run,
  ),
  verb(
    toolu_hub::ast_grep::PLUGIN,
    toolu_hub::ast_grep::command,
    toolu_hub::ast_grep::run,
  ),
  verb(
    toolu_brainstorm::PLUGIN,
    toolu_brainstorm::command,
    toolu_brainstorm::run,
  ),
  verb(
    toolu_delivery_flow::PLUGIN,
    toolu_delivery_flow::command,
    toolu_delivery_flow::run,
  ),
  verb(
    toolu_review::PLUGIN,
    toolu_review::command,
    toolu_review::run,
  ),
  verb(toolu_jev::PLUGIN, toolu_jev::command, toolu_jev::run),
  verb(
    toolu_statusline::PLUGIN,
    toolu_statusline::command,
    toolu_statusline::run,
  ),
  verb(
    toolu_pr_babysit::PLUGIN,
    toolu_pr_babysit::command,
    toolu_pr_babysit::run,
  ),
  verb(
    toolu_epic_orchestrator::PLUGIN,
    toolu_epic_orchestrator::command,
    toolu_epic_orchestrator::run,
  ),
  Namespace {
    owner: BUILTIN,
    command: commands::command,
    action: Action::Commands,
  },
  verb(BUILTIN, plugins_command, plugins_run),
];

/// The namespace whose command is `name`.
pub(crate) fn find(name: &str) -> Option<&'static Namespace> {
  NAMESPACES
    .iter()
    .find(|namespace| (namespace.command)().get_name() == name)
}

/// Whether `owner` is a plugin other than toolu: its namespace carries a hidden
/// `hook` verb and, when the names differ, the plugin name as an alias.
pub(crate) fn is_leaf_plugin(owner: &str) -> bool {
  owner != toolu_hub::PLUGIN && owner != BUILTIN
}

/// Whether `word` is a leaf plugin's own name, the first word of its `hooks.json`
/// line (`toolu <plugin> hook <name>`). Reads the owners only: no clap command is built.
pub(crate) fn is_hook_owner(word: &str) -> bool {
  is_leaf_plugin(word) && NAMESPACES.iter().any(|namespace| namespace.owner == word)
}

#[cfg(test)]
#[path = "tests/registry_test.rs"]
mod tests;
