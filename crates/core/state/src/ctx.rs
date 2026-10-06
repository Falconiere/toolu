//! `StateCtx`: what every state writer takes (`StateOptions` in
//! `packages/toolu-core/src/state/state-io.ts`): the host-bound environment, a
//! preloaded config (else loaded once from the root), a clock, and the
//! warnings collected for the caller to print.

use std::path::Path;
use std::time::SystemTime;

use toolu_runtime::config::load::{LoadedConfig, load};
use toolu_runtime::host::roots::Roots;

/// The environment, config, clock and warnings of one state call chain.
#[derive(Debug)]
pub struct StateCtx {
  /// The environment bound to its host.
  pub roots: Roots,
  /// The config; loaded from the first root asked when `None`.
  pub config: Option<LoadedConfig>,
  /// A fixed clock; the system clock when `None`.
  pub now: Option<SystemTime>,
  /// Warnings, oldest first, for the caller to print (TypeScript's `warn`).
  pub warnings: Vec<String>,
}

impl StateCtx {
  /// `roots` with no preloaded config, the system clock and no warnings.
  pub fn new(roots: Roots) -> StateCtx {
    StateCtx {
      roots,
      config: None,
      now: None,
      warnings: Vec::new(),
    }
  }

  /// The fixed clock, else the system clock.
  pub fn now(&self) -> SystemTime {
    self.now.unwrap_or(SystemTime::now())
  }

  /// The config, loaded for `root` on first use; its warnings join ours.
  pub fn config(&mut self, root: &Path) -> &LoadedConfig {
    let config = self
      .config
      .get_or_insert_with(|| load(&self.roots, Some(root)));
    self.warnings.extend(config.take_warnings());
    config
  }
}

#[cfg(test)]
#[path = "tests/ctx_test.rs"]
mod tests;
