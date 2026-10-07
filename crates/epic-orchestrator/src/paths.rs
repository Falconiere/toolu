//! Files the resident engine owns under the resource root from #421.

use std::path::PathBuf;

use toolu_engine::resources::store::resource_home;
use toolu_runtime::env::Env;

/// Paths under one resource root.
#[derive(Clone)]
pub(crate) struct Paths {
  /// `resource_home`.
  pub root: PathBuf,
}

impl Paths {
  /// The resource root `env` selects.
  pub(crate) fn from_env(env: &Env) -> Self {
    Self {
      root: resource_home(env),
    }
  }

  /// An explicit root, used by tests and `--` nothing: the engine takes the root from env.
  pub(crate) fn at(root: impl Into<PathBuf>) -> Self {
    Self { root: root.into() }
  }

  /// The single-instance pid lock.
  pub(crate) fn lock(&self) -> PathBuf {
    self.root.join("engine.lock")
  }

  /// The control socket, mode 0600.
  pub(crate) fn socket(&self) -> PathBuf {
    self.root.join("engine.sock")
  }

  /// Daily journal files.
  pub(crate) fn journal_dir(&self) -> PathBuf {
    self.root.join("journal")
  }

  /// The append lock.
  pub(crate) fn journal_lock(&self) -> PathBuf {
    self.root.join("journal.lock")
  }

  /// Reports written when the engine cannot ack.
  pub(crate) fn spool(&self) -> PathBuf {
    self.root.join("spool")
  }

  /// Registered epics.
  pub(crate) fn registry(&self) -> PathBuf {
    self.root.join("registry.json")
  }

  /// Persisted pause.
  pub(crate) fn pause(&self) -> PathBuf {
    self.root.join("pause.json")
  }

  /// Checkpoint and herdr deadlines.
  pub(crate) fn watch(&self) -> PathBuf {
    self.root.join("watch.json")
  }

  /// systemd unit written by `service install`.
  pub(crate) fn service(&self) -> PathBuf {
    self.root.join("toolu-epic.service")
  }
}

#[cfg(test)]
#[path = "tests/paths_test.rs"]
mod tests;
