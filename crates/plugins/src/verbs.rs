//! The four verbs on a host driven through its own plugin CLI (Claude Code,
//! Codex, Muse).

use std::sync::Arc;
use std::time::Duration;

use toolu_runtime::env::Env;

use crate::args::Approval;
use crate::host::adapter::{Adapter, Installed};
use crate::host::checkout::Checkout;
use crate::host::run::{Ran, run};
use crate::progress::Progress;
use crate::step::{Status, Step};

/// Dependency-ordered install.
pub(crate) mod install;
/// The catalog joined with the host's state.
pub(crate) mod list;
/// Codex's marketplace pinned to the release tag.
pub(crate) mod pin;
/// The marketplace a host installs from, made ready before install and update.
pub(crate) mod prepare;
/// Uninstall.
pub(crate) mod remove;
/// Approval of installed hooks, on a host that asks for it.
pub(crate) mod trust;
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
  /// Muse's toolu checkout; `None` on the other hosts.
  pub(crate) checkout: Option<Checkout>,
  /// The release tag Codex's marketplace is pinned to; `None` on the other
  /// hosts.
  pub(crate) pin: Option<String>,
  /// What to do with hooks awaiting approval after an install or update.
  pub(crate) approval: Approval,
  /// The live progress at the terminal, when there is one.
  pub(crate) progress: Option<Arc<Progress>>,
}

impl Driver {
  /// Starts this host's progress over `total` plugins, at `task`.
  pub(crate) fn begin(&self, total: usize, task: &str) {
    if let Some(progress) = &self.progress {
      progress.begin(self.adapter.bin(), total, task);
    }
  }

  /// Names the plugin at work.
  pub(crate) fn working(&self, task: &str) {
    if let Some(progress) = &self.progress {
      progress.task(task);
    }
  }

  /// One plugin done.
  pub(crate) fn advance(&self) {
    if let Some(progress) = &self.progress {
      progress.advance();
    }
  }

  /// Clears this host's progress.
  pub(crate) fn end(&self) {
    if let Some(progress) = &self.progress {
      progress.end();
    }
  }

  /// What the marketplace offers; nothing on Claude Code, which lists no
  /// versions for what is installed.
  pub(crate) fn offered(&self) -> Vec<Installed> {
    self
      .adapter
      .list_available()
      .map(|argv| self.versions(&argv))
      .unwrap_or_default()
  }

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

  /// Whether the host installed `entry` from outside toolu's marketplace, which
  /// leaves it for the user to change (only Muse reports a source).
  pub(crate) fn foreign(&self, entry: &Installed) -> bool {
    match (&self.checkout, &entry.source) {
      (Some(checkout), Some(path)) => !checkout.owns(path),
      _unknown => false,
    }
  }
}

/// The step for a plugin the host installed from outside toolu's marketplace.
pub(crate) fn foreign_step(name: &str, have: &Installed, argv: Vec<String>) -> Step {
  let source = have.source.as_deref().unwrap_or("elsewhere");
  Step::new(
    name,
    Status::Skew,
    format!(
      "installed from {source}, outside the toolu marketplace; left untouched. Run `muse plugins remove {name}`, then `toolu plugins install {name} --host muse`."
    ),
    argv,
  )
}

/// The step for `name`, installed from toolu's marketplace but gone from the
/// catalog: left alone, naming the command that removes it on `host` (Muse
/// removes by bare name, so its own command, as `toolu doctor` names it).
pub(crate) fn retired_step(name: &str, host: &str, argv: Vec<String>) -> Step {
  let remove = match host {
    "muse" => format!("muse plugins remove {name}"),
    _plugin_cli_or_local => format!("toolu plugins remove {name} --host {host} --yes"),
  };
  Step::new(
    name,
    Status::Skew,
    format!("retired from the toolu marketplace; remove it with `{remove}`"),
    argv,
  )
}

/// The entry for `name` in a listing.
pub(crate) fn find<'a>(list: &'a [Installed], name: &str) -> Option<&'a Installed> {
  list.iter().find(|entry| entry.name == name)
}

#[cfg(test)]
#[path = "tests/verbs_test.rs"]
mod tests;
