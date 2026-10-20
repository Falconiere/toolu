//! Codex's `plugin list --json`: `{installed: [{name, version, enabled?}]}`.

use serde_json::Value;

use crate::host::adapter::Installed;

/// The plugins of a `codex plugin list --json` payload.
pub(crate) fn parse(stdout: &str) -> Result<Vec<Installed>, String> {
  let value: Value = serde_json::from_str(stdout).map_err(|err| err.to_string())?;
  let entries = value
    .get("installed")
    .and_then(Value::as_array)
    .ok_or("expected an object with an \"installed\" array")?;
  entries.iter().map(entry).collect()
}

fn entry(item: &Value) -> Result<Installed, String> {
  let text = |key: &str| {
    item
      .get(key)
      .and_then(Value::as_str)
      .map(str::to_owned)
      .ok_or(format!("a plugin has no string \"{key}\""))
  };
  Ok(Installed {
    name: text("name")?,
    version: text("version")?,
    enabled: item.get("enabled").and_then(Value::as_bool).unwrap_or(true),
    source: None,
  })
}

#[cfg(test)]
#[path = "tests/codex_test.rs"]
mod tests;
