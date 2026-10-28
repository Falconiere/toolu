//! `OpenCode`'s installed toolu plugins, read from disk: `opencode plugin list`
//! names packages, not the directories `toolu plugins` copies. A plugin it
//! installed sits in `<config>/plugins/<name>` with the install marker; one
//! without its `server.js` shim never loads, so it is named in a warning. So is
//! the deprecated `@toolu/opencode` package wherever the config still names it.

use std::path::{Path, PathBuf};

use serde_json::Value;
use toolu_protocol::launcher::opencode::FILE;
use toolu_runtime::host::cursor::MARKER;
use toolu_runtime::host::opencode::plugins_dir;
use toolu_runtime::host::roots::Roots;

use super::inventory::{Inventory, Plugin};

/// The deprecated package `OpenCode` loaded before `toolu plugins` did.
const LEGACY: &str = "@toolu/opencode";

/// The toolu plugins `OpenCode` has, and a warning naming any without its shim
/// and any file that still names `@toolu/opencode`.
pub(super) fn collect(roots: &Roots) -> Inventory {
  let (found, unreadable) = ours(&plugins_dir(roots));
  let broken: Vec<&str> = found
    .iter()
    .filter(|(_, dir)| !dir.join(FILE).is_file())
    .map(|(name, _)| name.as_str())
    .collect();
  let mut warnings: Vec<String> = unreadable.into_iter().collect();
  if !broken.is_empty() {
    warnings.push(format!(
      "OpenCode does not load {} without {FILE}; run `toolu plugins update --host opencode`",
      broken.join(", ")
    ));
  }
  let legacy = legacy(&roots.config_root());
  if !legacy.is_empty() {
    let files: Vec<String> = legacy
      .iter()
      .map(|path| path.display().to_string())
      .collect();
    warnings.push(format!(
      "the deprecated {LEGACY} runs no toolu hooks; remove it from the OpenCode plugins it is \
       named in ({})",
      files.join(", ")
    ));
  }
  let warning = (!warnings.is_empty()).then(|| warnings.join("; "));
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

/// The files under the `OpenCode` config that name `@toolu/opencode`: its
/// `opencode.json(c)` and `package.json`, and the `package.json` of each directory
/// one level down, where a local bridge plugin that depends on it lives.
fn legacy(config: &Path) -> Vec<PathBuf> {
  let mut files: Vec<PathBuf> = ["opencode.json", "opencode.jsonc", "package.json"]
    .iter()
    .map(|name| config.join(name))
    .collect();
  if let Ok(entries) = std::fs::read_dir(config) {
    let mut nested: Vec<PathBuf> = entries
      .filter_map(Result::ok)
      .map(|entry| entry.path().join("package.json"))
      .collect();
    nested.sort();
    files.extend(nested);
  }
  files
    .into_iter()
    .filter(|file| std::fs::read_to_string(file).is_ok_and(|text| text.contains(LEGACY)))
    .collect()
}

/// The plugin directories toolu installed (they carry its marker), sorted, and
/// why the directory could not be read in full; a missing directory has none.
fn ours(dir: &Path) -> (Vec<(String, PathBuf)>, Option<String>) {
  let entries = match std::fs::read_dir(dir) {
    Ok(entries) => entries,
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => return (Vec::new(), None),
    Err(err) => {
      return (
        Vec::new(),
        Some(format!("cannot read {}: {err}", dir.display())),
      );
    }
  };
  let mut found = Vec::new();
  let mut unreadable = None;
  for entry in entries {
    match entry {
      Ok(entry) if entry.path().join(MARKER).is_file() => found.push((
        entry.file_name().to_string_lossy().into_owned(),
        entry.path(),
      )),
      Ok(_) => {}
      Err(err) => unreadable = Some(format!("cannot read {}: {err}", dir.display())),
    }
  }
  found.sort();
  (found, unreadable)
}

fn version_of(root: &Path) -> Option<String> {
  let text = std::fs::read_to_string(root.join(".claude-plugin/plugin.json")).ok()?;
  let json: Value = serde_json::from_str(&text).ok()?;
  json.get("version")?.as_str().map(str::to_owned)
}

#[cfg(test)]
#[path = "tests/opencode_test.rs"]
mod tests;
