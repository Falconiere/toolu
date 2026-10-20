//! Claude Code's `plugin list --json`: an array of `{id, version, enabled?}`,
//! where `id` is `<name>@<marketplace>`.

use serde_json::Value;

use crate::host::adapter::Installed;

/// The plugins of a `claude plugin list --json` payload.
pub(crate) fn parse(stdout: &str) -> Result<Vec<Installed>, String> {
  let value: Value = serde_json::from_str(stdout).map_err(|err| err.to_string())?;
  let entries = value.as_array().ok_or("expected an array of plugins")?;
  entries.iter().map(entry).collect()
}

fn entry(item: &Value) -> Result<Installed, String> {
  let id = item
    .get("id")
    .and_then(Value::as_str)
    .ok_or("a plugin has no string \"id\"")?;
  let version = item
    .get("version")
    .and_then(Value::as_str)
    .ok_or("a plugin has no string \"version\"")?;
  let name = match id.rsplit_once('@') {
    Some((name, _marketplace)) if !name.is_empty() => name,
    _no_marketplace => id,
  };
  Ok(Installed {
    name: name.to_owned(),
    version: version.to_owned(),
    enabled: item.get("enabled").and_then(Value::as_bool).unwrap_or(true),
    source: None,
  })
}

#[cfg(test)]
#[path = "tests/claude_test.rs"]
mod tests;
