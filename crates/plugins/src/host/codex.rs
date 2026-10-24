//! Codex's `plugin list --json` (`{installed: [{name, version, enabled?}]}`)
//! and its marketplace commands, which pin toolu's marketplace to a release
//! tag (#626).

use serde_json::Value;

use crate::host::adapter::{Installed, MARKETPLACE};

/// The toolu plugins of a `codex plugin list --json` payload. Codex lists
/// every marketplace's plugins; an entry naming another marketplace is not
/// toolu's, even under a toolu plugin's name.
pub(crate) fn parse(stdout: &str) -> Result<Vec<Installed>, String> {
  let value: Value = serde_json::from_str(stdout).map_err(|err| err.to_string())?;
  let entries = value
    .get("installed")
    .and_then(Value::as_array)
    .ok_or("expected an object with an \"installed\" array")?;
  entries
    .iter()
    .filter(|item| {
      item
        .get("marketplaceName")
        .and_then(Value::as_str)
        .is_none_or(|name| name == MARKETPLACE)
    })
    .map(entry)
    .collect()
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

/// Where a Codex marketplace comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Source {
  /// `git` or `local`.
  pub(crate) kind: String,
  /// The URL or directory.
  pub(crate) source: String,
}

/// The source of the marketplace `name` in a `codex plugin marketplace list
/// --json` payload, or `None` when Codex has no such marketplace.
pub(crate) fn marketplace_source(stdout: &str, name: &str) -> Result<Option<Source>, String> {
  let value: Value = serde_json::from_str(stdout).map_err(|err| err.to_string())?;
  let registered = value
    .get("marketplaces")
    .and_then(Value::as_array)
    .ok_or("expected an object with a \"marketplaces\" array")?;
  let Some(entry) = registered
    .iter()
    .find(|entry| entry.get("name").and_then(Value::as_str) == Some(name))
  else {
    return Ok(None);
  };
  let source = entry
    .get("marketplaceSource")
    .ok_or(format!("marketplace {name} has no \"marketplaceSource\""))?;
  let text = |key: &str| {
    source
      .get(key)
      .and_then(Value::as_str)
      .map(str::to_owned)
      .ok_or(format!("marketplace {name} has no string \"{key}\""))
  };
  Ok(Some(Source {
    kind: text("sourceType")?,
    source: text("source")?,
  }))
}

/// `codex plugin marketplace list --json`.
pub(crate) fn marketplace_list() -> Vec<String> {
  argv(&["plugin", "marketplace", "list", "--json"])
}

/// `codex plugin marketplace remove toolu`.
pub(crate) fn marketplace_remove() -> Vec<String> {
  argv(&["plugin", "marketplace", "remove", MARKETPLACE])
}

fn argv(words: &[&str]) -> Vec<String> {
  std::iter::once("codex")
    .chain(words.iter().copied())
    .map(str::to_owned)
    .collect()
}

#[cfg(test)]
#[path = "tests/codex_test.rs"]
mod tests;
