//! Missing-plugin warning (`dependency-warning.ts`). An indeterminate install
//! record silences the whole block.

use std::path::{Path, PathBuf};

use serde_json::Value;
use toolu_protocol::host::Host;
use toolu_runtime::config::load::is_file;
use toolu_runtime::env::Env;
use toolu_runtime::host::snapshot::Installed;
use toolu_runtime::json::stringify;

use crate::lifecycle::presence::{install_command, presence};

/// The WARN block, or nothing when no dependency is definitively missing.
pub(crate) fn dependency_warning(
  env: &Env,
  host: Host,
  plugin_root: &str,
  project_root: &str,
) -> Option<String> {
  let path = manifest_path(plugin_root, project_root, host)?;
  let doc = read_json(&path).unwrap_or(Value::Null);
  let mut missing = Vec::new();
  for spec in dependency_specs(&doc) {
    match presence(&spec, env, host) {
      Installed::Unknown => return None,
      Installed::Absent => missing.push(install_command(&spec, host)),
      Installed::Installed => {}
    }
  }
  if missing.is_empty() {
    return None;
  }
  let mut lines = String::new();
  for command in &missing {
    lines.push_str("\n  • ");
    lines.push_str(command);
  }
  Some(format!(
    "WARN: required plugins missing — dependent workflows will fail. Install:{lines}"
  ))
}

/// Spec lines, empty lines skipped, embedded newlines split.
pub(crate) fn dependency_specs(manifest: &Value) -> Vec<String> {
  entries(manifest)
    .into_iter()
    .filter_map(spec_of)
    .flat_map(|spec| spec_lines(&spec))
    .collect()
}

fn entries(manifest: &Value) -> Vec<&Value> {
  let Some(deps) = manifest.get("dependencies") else {
    return Vec::new();
  };
  if let Some(list) = deps.as_array() {
    return list.iter().collect();
  }
  deps
    .as_object()
    .map(|map| map.values().collect())
    .unwrap_or_default()
}

fn spec_of(entry: &Value) -> Option<String> {
  if let Some(text) = entry.as_str() {
    return Some(text.to_owned());
  }
  let object = entry.as_object()?;
  let name = object.get("name")?.as_str()?;
  match object.get("marketplace") {
    None | Some(Value::Null | Value::Bool(false)) => Some(name.to_owned()),
    Some(market) => Some(format!("{name}@{}", interpolate(market))),
  }
}

fn spec_lines(spec: &str) -> Vec<String> {
  spec
    .split('\n')
    .filter(|line| !line.is_empty())
    .map(str::to_owned)
    .collect()
}

fn interpolate(value: &Value) -> String {
  value
    .as_str()
    .map_or_else(|| stringify(value), str::to_owned)
}

fn manifest_path(plugin_root: &str, project_root: &str, host: Host) -> Option<PathBuf> {
  let claude = Path::new(plugin_root)
    .join(".claude-plugin")
    .join("plugin.json");
  if let Some(path) = codex_manifest(plugin_root, host, &claude) {
    return Some(path);
  }
  if !plugin_root.is_empty() && is_file(&claude) {
    return Some(claude);
  }
  let checkout = Path::new(project_root)
    .join("plugins")
    .join("toolu")
    .join(".claude-plugin")
    .join("plugin.json");
  is_file(&checkout).then_some(checkout)
}

fn codex_manifest(plugin_root: &str, host: Host, claude: &Path) -> Option<PathBuf> {
  if host != Host::Codex || plugin_root.is_empty() {
    return None;
  }
  let codex = Path::new(plugin_root)
    .join(".codex-plugin")
    .join("plugin.json");
  if !is_file(&codex) {
    return None;
  }
  let has_deps = read_json(&codex)
    .is_some_and(|doc| doc.get("dependencies").and_then(Value::as_array).is_some());
  if has_deps || !is_file(claude) {
    Some(codex)
  } else {
    Some(claude.to_path_buf())
  }
}

fn read_json(path: &Path) -> Option<Value> {
  let text = std::fs::read_to_string(path).ok()?;
  serde_json::from_str(&text).ok()
}

#[cfg(test)]
#[path = "tests/dependencies_test.rs"]
mod tests;
