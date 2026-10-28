//! `OpenCode`'s installed toolu plugins, read from disk: `opencode plugin list`
//! names packages, not the directories `toolu plugins` copies. A plugin it
//! installed sits in `<config>/plugins/<name>` with the install marker; one
//! without its `server.js` shim never loads, so it is named in a warning.

use std::path::{Path, PathBuf};

use serde_json::Value;
use toolu_protocol::launcher::opencode::FILE;
use toolu_runtime::host::cursor::MARKER;
use toolu_runtime::host::opencode::plugins_dir;
use toolu_runtime::host::roots::Roots;

use super::inventory::{Inventory, Plugin};

/// The toolu plugins `OpenCode` has, and a warning naming any without its shim.
pub(super) fn collect(roots: &Roots) -> Inventory {
  let found = ours(&plugins_dir(roots));
  let broken: Vec<&str> = found
    .iter()
    .filter(|(_, dir)| !dir.join(FILE).is_file())
    .map(|(name, _)| name.as_str())
    .collect();
  let warning = (!broken.is_empty()).then(|| {
    format!(
      "OpenCode does not load {} without {FILE}; run `toolu plugins update --host opencode`",
      broken.join(", ")
    )
  });
  let plugins = found
    .iter()
    .map(|(name, root)| Plugin {
      version: version_of(root),
      name: name.clone(),
      root: Some(root.clone()),
      muse: None,
    })
    .collect();
  Inventory { plugins, warning }
}

/// The plugin directories toolu installed (they carry its marker), sorted.
fn ours(dir: &Path) -> Vec<(String, PathBuf)> {
  let Ok(entries) = std::fs::read_dir(dir) else {
    return Vec::new();
  };
  let mut found: Vec<(String, PathBuf)> = entries
    .filter_map(Result::ok)
    .filter(|entry| entry.path().join(MARKER).is_file())
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

fn version_of(root: &Path) -> Option<String> {
  let text = std::fs::read_to_string(root.join(".claude-plugin/plugin.json")).ok()?;
  let json: Value = serde_json::from_str(&text).ok()?;
  json.get("version")?.as_str().map(str::to_owned)
}

#[cfg(test)]
#[path = "tests/opencode_test.rs"]
mod tests;
