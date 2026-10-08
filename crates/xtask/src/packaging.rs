//! `cargo xtask packaging`: plugin manifests, marketplaces, and release lockstep.

use std::path::Path;

use serde_json::Value;

use crate::options::Options;
use crate::packaging_assets::{self, PLUGINS, dirs, read_json};
use crate::packaging_catalog;
use crate::{Verdict, output};

const WORKSPACE_PACKAGES: [&str; 3] = [
  "packages/toolu-core/package.json",
  "tools/toolu-opencode/package.json",
  "tools/toolu-conformance/package.json",
];

/// Validate plugin packaging under `--root`.
pub(crate) fn run(options: &Options) -> Result<Verdict, String> {
  if !options.root.is_dir() {
    return Err(format!("missing root {}", options.root.display()));
  }
  match package(&options.root) {
    Ok((plugins, skills, agents, hooks)) => {
      output::say(&format!(
        "validate-plugin-packaging: validated {plugins} plugins, {skills} skills, {agents} agents, and {hooks} hook manifests"
      ));
      Ok(Verdict::Clean)
    }
    Err(message) => {
      let line = format!("validate-plugin-packaging: {message}");
      output::say(&line);
      output::error(&line);
      Ok(Verdict::Findings)
    }
  }
}

fn package(root: &Path) -> Result<(usize, usize, usize, usize), String> {
  let version = read_json(root, "package.json")?
    .get("version")
    .and_then(Value::as_str)
    .ok_or_else(|| "package.json must contain a version".to_owned())?
    .to_owned();
  let plugins = check_plugins(root, &version)?;
  let skills = packaging_assets::check_skills(root)?;
  let agents = packaging_assets::check_agents(root)?;
  let hooks = packaging_assets::check_hooks(root)?;
  Ok((plugins, skills, agents, hooks))
}

fn check_plugins(root: &Path, version: &str) -> Result<usize, String> {
  let claude_catalog = read_json(root, ".claude-plugin/marketplace.json")?;
  let codex_catalog = read_json(root, ".agents/plugins/marketplace.json")?;
  let release = read_json(root, "release-please-config.json")?;
  let mut count = 0;
  for plugin_root in dirs(root, "plugins")? {
    let manifest = format!("{plugin_root}/.claude-plugin/plugin.json");
    if !root.join(&manifest).is_file() {
      continue;
    }
    let name = plugin_root
      .strip_prefix("plugins/")
      .ok_or_else(|| format!("plugin path {plugin_root}"))?;
    check_manifests(root, &plugin_root, name, version, &release)?;
    packaging_catalog::check_catalogs(
      name,
      &read_json(root, &manifest)?,
      &claude_catalog,
      &codex_catalog,
    )?;
    count += 1;
  }
  for pkg in WORKSPACE_PACKAGES {
    if read_json(root, pkg)?.get("version").and_then(Value::as_str) != Some(version) {
      return Err(format!("{pkg} version differs from package.json"));
    }
    if !packaging_catalog::release_tracks(&release, pkg, "json", "$.version") {
      return Err(format!("release-please is missing {pkg}"));
    }
  }
  packaging_catalog::check_cargo(root, version, &release)?;
  if count != PLUGINS {
    return Err(format!("expected {PLUGINS} plugins, found {count}"));
  }
  if packaging_catalog::array_len(&claude_catalog, "plugins") != count {
    return Err("Claude marketplace count does not match plugin manifests".to_owned());
  }
  if packaging_catalog::array_len(&codex_catalog, "plugins") != count {
    return Err("Codex marketplace count does not match plugin manifests".to_owned());
  }
  Ok(count)
}

fn check_manifests(
  root: &Path,
  plugin_root: &str,
  name: &str,
  version: &str,
  release: &Value,
) -> Result<(), String> {
  let claude_path = format!("{plugin_root}/.claude-plugin/plugin.json");
  let codex_path = format!("{plugin_root}/.codex-plugin/plugin.json");
  if !root.join(&codex_path).is_file() {
    return Err(format!("{name} is missing its Codex manifest"));
  }
  packaging_catalog::check_symlinks(root, plugin_root, name)?;
  let claude = read_json(root, &claude_path)?;
  let codex = read_json(root, &codex_path)?;
  if identity(&claude) != identity(&codex) {
    return Err(format!(
      "{name} has mismatched Claude/Codex identity metadata"
    ));
  }
  if claude.get("version").and_then(Value::as_str) != Some(version) {
    return Err(format!(
      "{name} Claude manifest version differs from package.json"
    ));
  }
  if codex.get("version").and_then(Value::as_str) != Some(version) {
    return Err(format!(
      "{name} Codex manifest version differs from package.json"
    ));
  }
  for manifest in [&claude_path, &codex_path] {
    if !packaging_catalog::release_tracks(release, manifest, "json", "$.version") {
      return Err(format!("release-please is missing {manifest}"));
    }
  }
  skills_decl(root, plugin_root, name, &codex)?;
  hooks_decl(root, plugin_root, name, &codex)
}

fn skills_decl(root: &Path, plugin_root: &str, name: &str, codex: &Value) -> Result<(), String> {
  let present = root.join(plugin_root).join("skills").exists();
  let declared = codex
    .as_object()
    .is_some_and(|obj| obj.contains_key("skills"));
  if present && codex.get("skills").and_then(Value::as_str) != Some("./skills/") {
    return Err(format!("{name} must declare ./skills/"));
  }
  if !present && declared {
    return Err(format!("{name} declares skills without a skills directory"));
  }
  Ok(())
}

fn hooks_decl(root: &Path, plugin_root: &str, name: &str, codex: &Value) -> Result<(), String> {
  let present = root.join(plugin_root).join("hooks/hooks.json").exists();
  let declared = codex
    .as_object()
    .is_some_and(|obj| obj.contains_key("hooks"));
  if present && codex.get("hooks").and_then(Value::as_str) != Some("./hooks/hooks.json") {
    return Err(format!("{name} must declare ./hooks/hooks.json"));
  }
  if !present && declared {
    return Err(format!("{name} declares hooks without hooks/hooks.json"));
  }
  Ok(())
}

fn identity(doc: &Value) -> String {
  let values =
    ["name", "version", "description"].map(|key| doc.get(key).cloned().unwrap_or(Value::Null));
  serde_json::to_string(&values).unwrap_or_else(|_| "[]".to_owned())
}

#[cfg(test)]
#[path = "tests/packaging_test.rs"]
mod tests;
