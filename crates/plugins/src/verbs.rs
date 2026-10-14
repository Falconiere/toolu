//! The four verbs on a host driven through its own plugin CLI (Claude Code,
//! Codex).

use std::time::Duration;

use toolu_runtime::env::Env;

use crate::host::adapter::{Adapter, Installed};
use crate::host::run::{Ran, run};

/// Dependency-ordered install.
pub(crate) mod install;
/// The catalog joined with the host's state.
pub(crate) mod list;
/// Uninstall.
pub(crate) mod remove;
/// Update what is behind.
pub(crate) mod update;

/// A host CLI and how to run it.
#[derive(Debug, Clone)]
pub(crate) struct Driver {
  /// The host's commands.
  pub(crate) adapter: Adapter,
  /// The environment its commands get.
  pub(crate) env: Env,
  /// The deadline of each command.
  pub(crate) timeout: Duration,
}

impl Driver {
  /// Runs one host command.
  pub(crate) fn run(&self, argv: &[String]) -> Ran {
    run(argv, &self.env, self.timeout)
  }

  /// What a listing reports, in the host's order; a failed or malformed
  /// listing is empty, so a plugin the host does not report is still attempted.
  pub(crate) fn versions(&self, argv: &[String]) -> Vec<Installed> {
    let ran = self.run(argv);
    if !ran.ok() {
      return Vec::new();
    }
    self.adapter.parse_list(&ran.stdout).unwrap_or_default()
  }
}

/// The entry for `name` in a listing.
pub(crate) fn find<'a>(list: &'a [Installed], name: &str) -> Option<&'a Installed> {
  list.iter().find(|entry| entry.name == name)
}

#[cfg(test)]
#[path = "tests/verbs_test.rs"]
mod tests;
