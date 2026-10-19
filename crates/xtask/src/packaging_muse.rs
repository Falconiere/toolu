//! Muse bundles for `cargo xtask packaging` (#548). Muse rejects the `agents`
//! capability, so each `plugins/<name>/agents/<agent>.md` ships as the Muse
//! command `{"id":"<agent>","path":"agents/<agent>.md"}`: the entry names the
//! Claude/Codex source itself, and no second copy exists to drift. A plugin's
//! `.muse-plugin/plugin.json` also moves in lockstep with the workspace version
//! and carries only the fields Muse validates without a warning.

use std::path::Path;

use serde_json::Value;

use crate::packaging_assets::{dirs, read_json};
use crate::packaging_catalog::release_tracks;

/// The top-level manifest fields Muse accepts without an `unsupported-field` warning.
const FIELDS: [&str; 7] = [
  "schemaVersion",
  "name",
  "displayName",
  "version",
  "description",
  "compat",
  "capabilities",
];

/// Check every plugin's Muse bundle; returns the number of agent commands checked.
pub(crate) fn check_muse(root: &Path, version: &str, release: &Value) -> Result<usize, String> {
  let mut count = 0;
  for plugin in dirs(root, "plugins")? {
    let agents = agents(root, &plugin)?;
    let manifest = format!("{plugin}/.muse-plugin/plugin.json");
    if !root.join(&manifest).is_file() {
      if agents.is_empty() {
        continue;
      }
      return Err(format!(
        "{plugin} has agents/ but no .muse-plugin/plugin.json"
      ));
    }
    let doc = read_json(root, &manifest)?;
    check_manifest(&manifest, &doc, version, release)?;
    check_commands(&manifest, &doc, &agents)?;
    count += agents.len();
  }
  Ok(count)
}

/// The sorted stems of `<plugin>/agents/*.md`.
fn agents(root: &Path, plugin: &str) -> Result<Vec<String>, String> {
  let dir = root.join(plugin).join("agents");
  if !dir.is_dir() {
    return Ok(Vec::new());
  }
  let listing =
    std::fs::read_dir(&dir).map_err(|err| format!("cannot list {plugin}/agents: {err}"))?;
  let mut stems = Vec::new();
  for entry in listing {
    let entry = entry.map_err(|err| format!("cannot list {plugin}/agents: {err}"))?;
    let name = entry.file_name().to_string_lossy().into_owned();
    if let Some(stem) = name.strip_suffix(".md") {
      stems.push(stem.to_owned());
    }
  }
  stems.sort();
  Ok(stems)
}

/// Lockstep version, release tracking, known fields and no `agents` capability.
fn check_manifest(
  manifest: &str,
  doc: &Value,
  version: &str,
  release: &Value,
) -> Result<(), String> {
  if doc.get("version").and_then(Value::as_str) != Some(version) {
    return Err(format!("{manifest} version differs from package.json"));
  }
  if !release_tracks(release, manifest, "json", "$.version") {
    return Err(format!("release-please is missing {manifest}"));
  }
  let unknown = doc
    .as_object()
    .and_then(|fields| fields.keys().find(|key| !FIELDS.contains(&key.as_str())));
  if let Some(key) = unknown {
    return Err(format!(
      "{manifest}: top-level {key} is not a Muse manifest field; Muse warns unsupported-field"
    ));
  }
  if doc.pointer("/capabilities/agents").is_some() {
    return Err(format!(
      "{manifest}: Muse rejects the agents capability; map each agent to a command"
    ));
  }
  Ok(())
}

/// One command per agent naming its source, and no `agents/` entry without one.
fn check_commands(manifest: &str, doc: &Value, agents: &[String]) -> Result<(), String> {
  let commands = match doc.pointer("/capabilities/commands") {
    None => &[][..],
    Some(Value::Array(commands)) => commands.as_slice(),
    Some(_) => return Err(format!("{manifest}: capabilities.commands is not an array")),
  };
  for entry in commands {
    let (id, path) = (field(entry, "id"), field(entry, "path"));
    let Some(stem) = path
      .strip_prefix("agents/")
      .and_then(|file| file.strip_suffix(".md"))
    else {
      continue;
    };
    if !agents.iter().any(|agent| agent == stem) {
      return Err(format!(
        "{manifest}: command {id} points at {path}, which is not an agent source"
      ));
    }
    if id != stem {
      return Err(format!(
        "{manifest}: command {id} must be named {stem} after {path}"
      ));
    }
  }
  for agent in agents {
    let path = format!("agents/{agent}.md");
    let mapped = commands
      .iter()
      .any(|entry| field(entry, "id") == agent && field(entry, "path") == path);
    if !mapped {
      return Err(format!(
        "{manifest}: {path} has no Muse command {{\"id\":\"{agent}\",\"path\":\"{path}\"}}"
      ));
    }
  }
  Ok(())
}

/// A command entry's string field, or empty when absent.
fn field<'a>(entry: &'a Value, key: &str) -> &'a str {
  entry.get(key).and_then(Value::as_str).unwrap_or("")
}

#[cfg(test)]
#[path = "tests/packaging_muse_test.rs"]
mod tests;
