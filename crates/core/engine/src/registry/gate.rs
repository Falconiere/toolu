//! Installed-plugin gating (`registry-gate.ts`, a port of `detect_plugin_installed`
//! and `toolu_plugin_active`). A module runs unless its plugin is definitively
//! absent: an unreadable install record fails open, so a moved or malformed file
//! never silently switches enforcement off.

use std::path::PathBuf;

use serde_json::Value;
use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::host::snapshot::{Installed, codex_plugin_installed};

/// Whether plugin `spec` (`name@marketplace`) is installed. Claude reads
/// `installed_plugins.json`, Codex its `SessionStart` snapshot; Cursor, Hermes and
/// `OpenCode` keep no record toolu can read, so they are `Unknown`.
pub fn plugin_presence(spec: &str, roots: &Roots) -> Installed {
  if spec.is_empty() {
    return Installed::Absent;
  }
  match roots.host() {
    Host::Codex => codex_plugin_installed(spec, roots),
    Host::Claude => claude_presence(spec, roots.env()),
    Host::Opencode | Host::Cursor | Host::Hermes => Installed::Unknown,
  }
}

/// Registry modules of `spec` run unless the plugin is definitively absent.
pub fn plugin_active(spec: &str, roots: &Roots) -> bool {
  plugin_presence(spec, roots) != Installed::Absent
}

/// Claude Code's install record: `CLAUDE_PLUGINS_REGISTRY`, else
/// `<TOOLU_CONFIG_DIR | CLAUDE_CONFIG_DIR | $HOME/.claude>/plugins/installed_plugins.json`.
fn installed_plugins_path(env: &Env) -> PathBuf {
  if let Some(path) = env.get("CLAUDE_PLUGINS_REGISTRY") {
    return PathBuf::from(path);
  }
  let root = env
    .get("TOOLU_CONFIG_DIR")
    .or_else(|| env.get("CLAUDE_CONFIG_DIR"))
    .map_or_else(|| env.home().join(".claude"), PathBuf::from);
  root.join("plugins").join("installed_plugins.json")
}

fn claude_presence(spec: &str, env: &Env) -> Installed {
  let path = installed_plugins_path(env);
  let regular = std::fs::metadata(&path).is_ok_and(|meta| meta.is_file());
  let doc = regular
    .then(|| std::fs::read_to_string(&path).ok())
    .flatten()
    .and_then(|text| serde_json::from_str::<Value>(&text).ok());
  match doc.as_ref().and_then(|doc| doc.get("plugins")?.as_object()) {
    Some(plugins) if plugins.contains_key(spec) => Installed::Installed,
    Some(_) => Installed::Absent,
    None => Installed::Unknown,
  }
}

#[cfg(test)]
#[path = "tests/gate_test.rs"]
mod tests;
