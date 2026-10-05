//! The calling plugin's manifest: its `version` and its `hookProtocol` (#411).

use std::io::ErrorKind;
use std::path::Path;

use serde_json::Value;

/// What a hook needs from `plugin.json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
  /// The plugin's semver; empty when absent or not a string.
  pub version: String,
  /// The hook-interface version the plugin was written for.
  pub hook_protocol: u32,
}

/// Read `<root>/.claude-plugin/plugin.json`, or `<root>/.codex-plugin/plugin.json`
/// when the first does not exist. A first file that exists but is unreadable or
/// bad is an error, never a fallback.
///
/// # Errors
/// When `root` is empty, no manifest exists or reads, it is not JSON, or
/// `hookProtocol` is not a JSON integer from 1 to `u32::MAX`.
pub fn read(root: &Path) -> Result<Manifest, String> {
  if root.as_os_str().is_empty() {
    return Err("the plugin root is empty".to_owned());
  }
  let claude = root.join(".claude-plugin/plugin.json");
  let absent = matches!(std::fs::metadata(&claude), Err(err) if err.kind() == ErrorKind::NotFound);
  let path = if absent {
    root.join(".codex-plugin/plugin.json")
  } else {
    claude
  };
  let text = std::fs::read_to_string(&path)
    .map_err(|err| format!("cannot read {}: {err}", path.display()))?;
  let json: Value =
    serde_json::from_str(&text).map_err(|err| format!("{} is not JSON: {err}", path.display()))?;
  let hook_protocol = json
    .get("hookProtocol")
    .and_then(Value::as_u64)
    .filter(|protocol| *protocol >= 1)
    .and_then(|protocol| u32::try_from(protocol).ok())
    .ok_or_else(|| {
      format!(
        "{} has no integer hookProtocol of 1 or more",
        path.display()
      )
    })?;
  let version = json
    .get("version")
    .and_then(Value::as_str)
    .unwrap_or_default()
    .to_owned();
  Ok(Manifest {
    version,
    hook_protocol,
  })
}

#[cfg(test)]
#[path = "tests/manifest_test.rs"]
mod tests;
