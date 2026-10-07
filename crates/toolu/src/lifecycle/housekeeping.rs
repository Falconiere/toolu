//! What session-start does before it builds context (`session-housekeeping.ts`).
//! It runs even when the hook's context is disabled.

use toolu_engine::registry::prune::prune_inactive_modules;
use toolu_protocol::host::Host;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::host::snapshot::snapshot_codex_plugins;

/// Refresh Codex's plugin snapshot, prune its registry, and drop the legacy symlink.
pub(crate) fn housekeeping(roots: &Roots) {
  if roots.host() == Host::Codex {
    let _snapshot = snapshot_codex_plugins(roots);
    let _pruned = prune_inactive_modules(roots);
  }
  let legacy = roots.config_root().join("toolu").join("statusline.sh");
  let symlink = std::fs::symlink_metadata(&legacy).is_ok_and(|meta| meta.file_type().is_symlink());
  if symlink {
    let _removed = std::fs::remove_file(legacy);
  }
}

#[cfg(test)]
#[path = "tests/housekeeping_test.rs"]
mod tests;
