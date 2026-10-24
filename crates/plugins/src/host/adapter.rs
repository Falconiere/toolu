//! Claude Code's, Codex's and Muse's plugin commands behind one shape, so the
//! verbs never branch on the host: each builds the same argv kinds and
//! normalizes its plugin listing into `Installed`.

use crate::args::Scope;
use crate::host::{claude, codex, muse};

/// One plugin as a host reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Installed {
  /// The plugin name, without `@<marketplace>`.
  pub(crate) name: String,
  /// The installed (or offered) version.
  pub(crate) version: String,
  /// Whether the host has it enabled.
  pub(crate) enabled: bool,
  /// Where Muse installed it from; `None` on the other hosts and for an offer.
  pub(crate) source: Option<String>,
}

/// A host driven through its own plugin CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Adapter {
  /// `claude plugin …`.
  Claude,
  /// `codex plugin …`.
  Codex,
  /// `muse plugins …`, from the local toolu checkout (#549).
  Muse,
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
      Adapter::Muse => "muse",
    }
  }

  /// Adds the toolu marketplace from GitHub (Claude Code and Codex; Muse adds
  /// its checkout in `verbs::prepare`). Codex takes `pin`, a release tag, so
  /// its startup auto-upgrade never moves the plugins past it (#626).
  pub(crate) fn add_marketplace(self, pin: Option<&str>) -> Vec<String> {
    let tail = match (self, pin) {
      (Adapter::Codex, Some(tag)) => vec!["--ref".to_owned(), tag.to_owned()],
      _unpinned => Vec::new(),
    };
    words(&[self.bin(), "plugin", "marketplace", "add", SOURCE], &tail)
  }

  /// Installs `name`; only Claude Code takes a scope.
  pub(crate) fn install(self, name: &str, scope: Option<Scope>) -> Vec<String> {
    let id = format!("{name}@{MARKETPLACE}");
    match (self, scope) {
      (Adapter::Muse, _any) => muse::argv(&["plugins", "install", &id, "--json"]),
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
      Adapter::Muse => return muse::argv(&["plugins", "remove", name, "--json"]),
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
      Adapter::Muse => return muse::argv(&["plugins", "update", name, "--json"]),
    };
    words(
      &[self.bin(), "plugin", verb],
      &[format!("{name}@{MARKETPLACE}")],
    )
  }

  /// The host's plugin noun: Muse says `plugins`.
  fn noun(self) -> &'static str {
    match self {
      Adapter::Claude | Adapter::Codex => "plugin",
      Adapter::Muse => "plugins",
    }
  }

  /// Lists what is installed.
  pub(crate) fn list_installed(self) -> Vec<String> {
    words(&[self.bin(), self.noun(), "list", "--json"], &[])
  }

  /// Lists what the marketplaces offer.
  pub(crate) fn list_available(self) -> Vec<String> {
    words(
      &[self.bin(), self.noun(), "list", "--available", "--json"],
      &[],
    )
  }

  /// Reads `name`'s trust state; `None` on a host without an approval step.
  pub(crate) fn inspect(self, name: &str) -> Option<Vec<String>> {
    match self {
      Adapter::Claude | Adapter::Codex => None,
      Adapter::Muse => Some(muse::argv(&["plugins", "inspect", name, "--json"])),
    }
  }

  /// Trusts `name`'s current hook and agent definitions; `None` on a host
  /// without an approval step.
  pub(crate) fn approve(self, name: &str) -> Option<Vec<String>> {
    match self {
      Adapter::Claude | Adapter::Codex => None,
      Adapter::Muse => Some(muse::argv(&["plugins", "approve", name, "--json"])),
    }
  }

  /// Normalizes a `plugin list --json` payload; the wrong shape is an error,
  /// never an empty list.
  pub(crate) fn parse_list(self, stdout: &str) -> Result<Vec<Installed>, String> {
    let parsed = match self {
      Adapter::Claude => claude::parse(stdout),
      Adapter::Codex => codex::parse(stdout),
      Adapter::Muse => muse::parse(stdout),
    };
    parsed.map_err(|detail| format!("{} {} list --json: {detail}", self.bin(), self.noun()))
  }
}

#[cfg(test)]
#[path = "tests/adapter_test.rs"]
mod tests;
