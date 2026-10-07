//! The Codex registry prune (`registry-prune.ts`, a port of
//! `toolu_registry_prune_inactive` that also covers ESM modules and manifests).
//! Codex keeps no install record on the hot path, so at `SessionStart` the
//! registry drops modules whose plugin is definitively absent from the ready
//! plugin snapshot. A missing, stale or malformed snapshot prunes nothing;
//! symlinks and un-namespaced files are never removed.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use toolu_protocol::host::Host;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::host::snapshot::{Installed, codex_plugin_installed};
use toolu_runtime::registry::{RegistryEvent, event_dir};

use super::sorted_names;

/// `?*__*.{sh,js,json}` as `registry.sh` globs it: the spec runs to the first `__`.
fn spec_of(file: &str) -> Option<&str> {
  let module = [".sh", ".js", ".json"]
    .iter()
    .any(|ext| file.ends_with(ext));
  if file.starts_with('.') || !module {
    return None;
  }
  file
    .split_once("__")
    .map(|(spec, _)| spec)
    .filter(|spec| !spec.is_empty())
}

/// A regular file, not following symlinks (`lstatSync(path).isFile()`).
fn is_regular_file(path: &Path) -> bool {
  std::fs::symlink_metadata(path).is_ok_and(|meta| meta.is_file())
}

/// On Codex, remove the registry modules of plugins the ready snapshot does not
/// list, in both event directories; returns the removed paths. A file that cannot
/// be removed stays (`rm -f` parity) and is gated at dispatch instead.
pub fn prune_inactive_modules(roots: &Roots) -> Vec<PathBuf> {
  if roots.host() != Host::Codex {
    return Vec::new();
  }
  let mut absent: HashMap<String, bool> = HashMap::new();
  let mut removed = Vec::new();
  for event in RegistryEvent::ALL {
    let dir = event_dir(roots, event);
    for file in sorted_names(&dir) {
      let Some(spec) = spec_of(&file) else {
        continue;
      };
      let path = dir.join(&file);
      let gone = *absent
        .entry(spec.to_owned())
        .or_insert_with(|| codex_plugin_installed(spec, roots) == Installed::Absent);
      if gone && is_regular_file(&path) && std::fs::remove_file(&path).is_ok() {
        removed.push(path);
      }
    }
  }
  removed
}

#[cfg(test)]
#[path = "tests/prune_test.rs"]
mod tests;
