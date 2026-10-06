//! The Codex plugin snapshot (`packages/toolu-core/src/host/host-snapshot.ts`):
//! written once at Codex `SessionStart` from `codex plugin list --json`, so hot
//! paths never spawn the CLI, and read as a tri-state where a stale or failed
//! snapshot is unknown, never absent. Byte-identical to the bash writer.

use std::path::PathBuf;

use serde::Serialize;
use serde_json::{Map, Value};
use toolu_protocol::host::Host;

use super::roots::Roots;
use crate::atomic::write_atomic;
use crate::process::commands::codex_plugin_list;

/// Whether the snapshot could be trusted when it was taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SnapshotStatus {
  /// The listing was read and understood.
  Ready,
  /// The listing was missing or not understood.
  Indeterminate,
}

/// The snapshot file: `{"version":1,"status":…,"plugins":[…]}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CodexPluginSnapshot {
  /// Always 1.
  pub version: u8,
  /// Whether `plugins` can be trusted.
  pub status: SnapshotStatus,
  /// Installed and enabled plugin ids, in UTF-16 order.
  pub plugins: Vec<String>,
}

/// What [`snapshot_codex_plugins`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotResult {
  /// Where the snapshot belongs.
  pub path: PathBuf,
  /// What was (or would have been) written.
  pub snapshot: CodexPluginSnapshot,
  /// Whether the write landed; readers see unknown otherwise.
  pub written: bool,
}

/// What a snapshot says about one plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Installed {
  /// Listed, installed and enabled.
  Installed,
  /// A trustworthy snapshot does not list it.
  Absent,
  /// No trustworthy snapshot.
  Unknown,
}

const INDETERMINATE: CodexPluginSnapshot = CodexPluginSnapshot {
  version: 1,
  status: SnapshotStatus::Indeterminate,
  plugins: Vec::new(),
};

/// `TOOLU_CODEX_PLUGIN_SNAPSHOT`, else `<config root>/toolu/codex-plugins.json`.
pub fn codex_plugin_snapshot_path(roots: &Roots) -> PathBuf {
  match roots.env().get("TOOLU_CODEX_PLUGIN_SNAPSHOT") {
    Some(path) => PathBuf::from(path),
    None => roots.config_root().join("toolu").join("codex-plugins.json"),
  }
}

/// A flag is on when the key is absent or exactly `true` (jq `has | not or == true`).
fn flag_on(entry: &Map<String, Value>, key: &str) -> bool {
  entry
    .get(key)
    .is_none_or(|value| value == &Value::Bool(true))
}

/// `.pluginId // "<name>@<marketplaceName>"`, kept only as a non-empty string.
fn entry_id(entry: &Map<String, Value>) -> Option<String> {
  let id = match entry.get("pluginId") {
    Some(Value::Null | Value::Bool(false)) | None => {
      let name = entry.get("name").and_then(Value::as_str)?;
      let market = entry.get("marketplaceName").and_then(Value::as_str)?;
      return Some(format!("{name}@{market}")).filter(|id| !id.is_empty());
    }
    Some(id) => id,
  };
  id.as_str().filter(|id| !id.is_empty()).map(str::to_owned)
}

/// The canonical snapshot of a listing; any shape jq would reject is indeterminate.
fn canonical(listing: Option<&str>) -> CodexPluginSnapshot {
  let parsed = listing.and_then(|text| serde_json::from_str::<Value>(text).ok());
  let Some(installed) = parsed
    .as_ref()
    .and_then(|value| value.get("installed")?.as_array())
  else {
    return INDETERMINATE;
  };
  let mut ids: Vec<String> = Vec::new();
  for item in installed {
    let Some(entry) = item.as_object() else {
      return INDETERMINATE;
    };
    let id = (flag_on(entry, "installed") && flag_on(entry, "enabled")).then(|| entry_id(entry));
    if let Some(id) = id.flatten().filter(|id| !ids.contains(id)) {
      ids.push(id);
    }
  }
  ids.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
  CodexPluginSnapshot {
    version: 1,
    status: SnapshotStatus::Ready,
    plugins: ids,
  }
}

/// Refreshes the snapshot on Codex; `None`, with nothing written, on any other host.
pub fn snapshot_codex_plugins(roots: &Roots) -> Option<SnapshotResult> {
  if roots.host() != Host::Codex {
    return None;
  }
  let path = codex_plugin_snapshot_path(roots);
  let snapshot = canonical(codex_plugin_list(roots.env()).as_deref());
  let body = serde_json::to_string(&snapshot).ok()? + "\n";
  let written = write_atomic(&path, &body);
  Some(SnapshotResult {
    path,
    snapshot,
    written,
  })
}

/// `Installed` or `Absent` from a ready snapshot; `Unknown` without a trustworthy one.
pub fn codex_plugin_installed(spec: &str, roots: &Roots) -> Installed {
  if spec.is_empty() {
    return Installed::Absent;
  }
  let text = std::fs::read_to_string(codex_plugin_snapshot_path(roots)).ok();
  let file = text.and_then(|text| serde_json::from_str::<Value>(&text).ok());
  let ready = file.as_ref().filter(|file| {
    file.get("version").and_then(Value::as_f64) == Some(1.0)
      && file.get("status").and_then(Value::as_str) == Some("ready")
  });
  match ready.and_then(|file| file.get("plugins")?.as_array()) {
    None => Installed::Unknown,
    Some(plugins) if plugins.iter().any(|id| id.as_str() == Some(spec)) => Installed::Installed,
    Some(_) => Installed::Absent,
  }
}

#[cfg(test)]
#[path = "tests/snapshot_test.rs"]
mod tests;
