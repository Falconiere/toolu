//! Cursor's installed toolu plugins, read from disk: `cursor-agent plugin` has
//! no list command. A plugin `toolu plugins` installed sits in
//! `plugins/local/<name>` with its marker; one installed from the toolu
//! marketplace is cached at `plugins/cache/toolu/<name>/<rev>` and counts once
//! Cursor finished writing it.

use std::path::{Path, PathBuf};

use serde_json::Value;
use toolu_runtime::host::cursor::{CACHE_COMPLETE, MARKER, cache_dir, local_dir};
use toolu_runtime::host::roots::Roots;

use super::inventory::{Inventory, Plugin};

/// The toolu plugins Cursor has, and a warning for each whose hooks cannot run.
pub(super) fn collect(roots: &Roots) -> Inventory {
  let mut plugins = Vec::new();
  for (name, root) in cached(&cache_dir(roots)).into_iter().chain(local(roots)) {
    if plugins.iter().all(|seen: &Plugin| seen.name != name) {
      plugins.push(Plugin {
        version: version_of(&root),
        name,
        root: Some(root),
        muse: None,
      });
    }
  }
  Inventory {
    plugins,
    warning: None,
  }
}

fn subdirs(dir: &Path) -> Vec<(String, PathBuf)> {
  let Ok(entries) = std::fs::read_dir(dir) else {
    return Vec::new();
  };
  let mut found: Vec<(String, PathBuf)> = entries
    .filter_map(Result::ok)
    .filter(|entry| entry.path().is_dir())
    .map(|entry| {
      (
        entry.file_name().to_string_lossy().into_owned(),
        entry.path(),
      )
    })
    .collect();
  found.sort();
  found
}

/// `<cache>/<plugin>/<newest complete rev>`.
fn cached(cache: &Path) -> Vec<(String, PathBuf)> {
  subdirs(cache)
    .into_iter()
    .filter_map(|(name, dir)| {
      let newest = subdirs(&dir)
        .into_iter()
        .map(|(_, rev)| rev)
        .filter(|rev| rev.join(CACHE_COMPLETE).is_file())
        .max_by_key(|rev| std::fs::metadata(rev).and_then(|meta| meta.modified()).ok());
      newest.map(|rev| (name, rev))
    })
    .collect()
}

/// Local plugin directories toolu installed (they carry its marker).
fn local(roots: &Roots) -> Vec<(String, PathBuf)> {
  subdirs(&local_dir(roots))
    .into_iter()
    .filter(|(_, dir)| dir.join(MARKER).is_file())
    .collect()
}

/// The `version` of a plugin directory's Claude-format manifest.
pub(super) fn version_of(root: &Path) -> Option<String> {
  let text = std::fs::read_to_string(root.join(".claude-plugin/plugin.json")).ok()?;
  let json: Value = serde_json::from_str(&text).ok()?;
  json.get("version")?.as_str().map(str::to_owned)
}

#[cfg(test)]
#[path = "tests/cursor_test.rs"]
mod tests;
