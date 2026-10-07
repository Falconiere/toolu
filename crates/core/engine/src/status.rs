//! `StatusSnapshot`: the repository, gate and plugin status that `toolu status`
//! reports. The hub owns it, statusline renders it, and `crates/cli` hands
//! statusline the hub's implementation.

use std::path::Path;

use serde_json::Value;
use toolu_runtime::host::roots::Roots;

use crate::LinkError;

/// The status of the repository holding a directory.
pub trait StatusSnapshot: Send + Sync {
  /// The `toolu status --json` document for `dir`, resolved against `roots`.
  ///
  /// # Errors
  /// [`LinkError`] when the snapshot could not be taken.
  fn snapshot(&self, roots: &Roots, dir: &Path) -> Result<Value, LinkError>;
}

#[cfg(test)]
#[path = "tests/status_test.rs"]
mod tests;
