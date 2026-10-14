//! The hook entries `cargo xtask bench-hooks` measures: one per
//! `<plugin>/<hook name>` of every plugin's `hooks.json`, the first
//! registration winning, sorted by id.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value};
use toolu_protocol::launcher::hook_name;

/// One hook entry and the launcher command hosts run for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HookEntry {
  /// `<plugin>/<hook name>`, the payload and budget key.
  pub(crate) id: String,
  /// The event it is first registered under.
  pub(crate) event: String,
  /// The plugin directory, exported to the hook as `CLAUDE_PLUGIN_ROOT`.
  pub(crate) root: PathBuf,
  /// The `hooks.json` command, run under `/bin/sh -c`.
  pub(crate) command: String,
}

/// Every entry under `<root>/plugins`.
pub(crate) fn discover(root: &Path) -> Result<Vec<HookEntry>, String> {
  let plugins = root.join("plugins");
  let mut dirs: Vec<PathBuf> = std::fs::read_dir(&plugins)
    .map_err(|err| format!("cannot list {}: {err}", plugins.display()))?
    .flatten()
    .map(|entry| entry.path())
    .filter(|path| path.join("hooks/hooks.json").is_file())
    .collect();
  dirs.sort();
  let mut found: Vec<HookEntry> = Vec::new();
  for dir in dirs {
    for entry in entries_of(&dir)? {
      if !found.iter().any(|known| known.id == entry.id) {
        found.push(entry);
      }
    }
  }
  found.sort_by(|left, right| left.id.cmp(&right.id));
  Ok(found)
}

fn entries_of(dir: &Path) -> Result<Vec<HookEntry>, String> {
  let file = dir.join("hooks/hooks.json");
  let text = std::fs::read_to_string(&file).map_err(|err| format!("{}: {err}", file.display()))?;
  let doc: Value =
    serde_json::from_str(&text).map_err(|err| format!("{}: {err}", file.display()))?;
  let plugin = dir
    .file_name()
    .and_then(|name| name.to_str())
    .ok_or_else(|| format!("{}: no plugin name", dir.display()))?;
  let events = doc
    .get("hooks")
    .and_then(Value::as_object)
    .cloned()
    .unwrap_or_default();
  let mut found = Vec::new();
  for (event, groups) in &events {
    for command in commands(groups) {
      let name = hook_name(&command).ok_or_else(|| {
        format!(
          "{}: a {event} hook is not the native launcher",
          file.display()
        )
      })?;
      found.push(HookEntry {
        id: format!("{plugin}/{name}"),
        event: event.clone(),
        root: dir.to_path_buf(),
        command,
      });
    }
  }
  Ok(found)
}

fn commands(groups: &Value) -> Vec<String> {
  let groups = groups.as_array().map(Vec::as_slice).unwrap_or_default();
  groups
    .iter()
    .filter_map(|group| group.get("hooks").and_then(Value::as_array))
    .flatten()
    .filter_map(|hook| hook.get("command").and_then(Value::as_str))
    .map(str::to_owned)
    .collect()
}

/// The payloads must cover exactly the discovered entries; `only` must name known ones.
pub(crate) fn select(
  entries: Vec<HookEntry>,
  payloads: &Map<String, Value>,
  only: &[String],
) -> Result<Vec<HookEntry>, String> {
  let missing: Vec<&str> = entries
    .iter()
    .filter(|entry| !payloads.contains_key(&entry.id))
    .map(|entry| entry.id.as_str())
    .collect();
  let stale: Vec<&str> = payloads
    .keys()
    .filter(|id| !entries.iter().any(|entry| &entry.id == *id))
    .map(String::as_str)
    .collect();
  if !missing.is_empty() || !stale.is_empty() {
    return Err(format!(
      "payloads: no payload for [{}]; payloads for unknown entries [{}]",
      missing.join(", "),
      stale.join(", ")
    ));
  }
  if let Some(unknown) = only
    .iter()
    .find(|id| !entries.iter().any(|entry| &entry.id == *id))
  {
    return Err(format!("--only names an unknown entry: {unknown}"));
  }
  Ok(
    entries
      .into_iter()
      .filter(|entry| only.is_empty() || only.contains(&entry.id))
      .collect(),
  )
}

#[cfg(test)]
#[path = "tests/bench_hooks_entries_test.rs"]
mod tests;
