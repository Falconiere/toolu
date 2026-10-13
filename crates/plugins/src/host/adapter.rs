//! Claude Code's and Codex's plugin commands behind one shape, so the verbs
//! never branch on the host: each builds the same argv kinds and normalizes
//! `plugin list --json` into `Installed`.

use crate::args::Scope;
use crate::host::{claude, codex};

/// One plugin as a host reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Installed {
  /// The plugin name, without `@<marketplace>`.
  pub(crate) name: String,
  /// The installed (or offered) version.
  pub(crate) version: String,
  /// Whether the host has it enabled.
  pub(crate) enabled: bool,
}

/// A host driven through its own plugin CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Adapter {
  /// `claude plugin …`.
  Claude,
  /// `codex plugin …`.
  Codex,
}

/// The marketplace every toolu plugin is installed from.
pub(crate) const MARKETPLACE: &str = "toolu";

/// Where the marketplace comes from.
pub(crate) const SOURCE: &str = "Falconiere/toolu";

fn words(head: &[&str], tail: &[String]) -> Vec<String> {
  head
    .iter()
    .map(|word| (*word).to_owned())
    .chain(tail.iter().cloned())
    .collect()
}

impl Adapter {
  /// The host binary.
  pub(crate) fn bin(self) -> &'static str {
    match self {
      Adapter::Claude => "claude",
      Adapter::Codex => "codex",
    }
  }

  /// Adds the toolu marketplace.
  pub(crate) fn add_marketplace(self) -> Vec<String> {
    words(&[self.bin(), "plugin", "marketplace", "add", SOURCE], &[])
  }

  /// Installs `name`; only Claude Code takes a scope.
  pub(crate) fn install(self, name: &str, scope: Option<Scope>) -> Vec<String> {
    let id = format!("{name}@{MARKETPLACE}");
    match (self, scope) {
      (Adapter::Claude, Some(scope)) => words(
        &["claude", "plugin", "install"],
        &[id, "--scope".to_owned(), scope.name().to_owned()],
      ),
      (Adapter::Claude, None) => words(&["claude", "plugin", "install"], &[id]),
      (Adapter::Codex, _any) => words(&["codex", "plugin", "add"], &[id]),
    }
  }

  /// Uninstalls `name`.
  pub(crate) fn remove(self, name: &str) -> Vec<String> {
    let verb = match self {
      Adapter::Claude => "uninstall",
      Adapter::Codex => "remove",
    };
    words(
      &[self.bin(), "plugin", verb],
      &[format!("{name}@{MARKETPLACE}")],
    )
  }

  /// Updates `name`: Codex re-adds it.
  pub(crate) fn update(self, name: &str) -> Vec<String> {
    let verb = match self {
      Adapter::Claude => "update",
      Adapter::Codex => "add",
    };
    words(
      &[self.bin(), "plugin", verb],
      &[format!("{name}@{MARKETPLACE}")],
    )
  }

  /// Lists what is installed.
  pub(crate) fn list_installed(self) -> Vec<String> {
    words(&[self.bin(), "plugin", "list", "--json"], &[])
  }

  /// Lists what the marketplaces offer.
  pub(crate) fn list_available(self) -> Vec<String> {
    words(
      &[self.bin(), "plugin", "list", "--available", "--json"],
      &[],
    )
  }

  /// Normalizes a `plugin list --json` payload; the wrong shape is an error,
  /// never an empty list.
  pub(crate) fn parse_list(self, stdout: &str) -> Result<Vec<Installed>, String> {
    let parsed = match self {
      Adapter::Claude => claude::parse(stdout),
      Adapter::Codex => codex::parse(stdout),
    };
    parsed.map_err(|detail| format!("{} plugin list --json: {detail}", self.bin()))
  }
}

#[cfg(test)]
#[path = "tests/adapter_test.rs"]
mod tests;
