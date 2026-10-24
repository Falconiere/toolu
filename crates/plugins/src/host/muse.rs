//! Muse's `muse plugins … --json` documents (#549), as Muse 1.4.11 prints them
//! (`fixtures/plugins/muse.json`). `list --json` is `{plugins: [{record}]}`, where
//! `record` is the install record Muse keeps even for a plugin that no longer
//! validates; `list --available --json` is `{available: [{name, version,
//! marketplace}]}` across every marketplace.

use serde_json::Value;

use crate::host::adapter::{Installed, MARKETPLACE};

/// The trust states that leave a runtime capability inert until approved.
const PENDING: [&str; 2] = ["review_needed", "modified"];

/// A marketplace's source as `muse plugins marketplace list --json` reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Source {
  /// `local-dir`, `local-file` or `git`.
  pub(crate) kind: String,
  /// The path or URL it was added from.
  pub(crate) path: String,
}

/// The plugins of a `muse plugins list [--available] --json` payload: the
/// installed records, or the toolu marketplace's offers.
pub(crate) fn parse(stdout: &str) -> Result<Vec<Installed>, String> {
  let value: Value = serde_json::from_str(stdout).map_err(|err| err.to_string())?;
  if let Some(plugins) = value.get("plugins").and_then(Value::as_array) {
    return plugins.iter().map(installed).collect();
  }
  let offers = value
    .get("available")
    .and_then(Value::as_array)
    .ok_or("expected an object with a \"plugins\" or \"available\" array")?;
  let mut list = Vec::new();
  for offer in offers {
    if offer.get("marketplace").and_then(Value::as_str) == Some(MARKETPLACE) {
      list.push(Installed {
        name: text(offer, "name")?,
        version: text(offer, "version")?,
        enabled: true,
        source: None,
      });
    }
  }
  Ok(list)
}

fn text(item: &Value, key: &str) -> Result<String, String> {
  item
    .get(key)
    .and_then(Value::as_str)
    .map(str::to_owned)
    .ok_or(format!("an entry has no string \"{key}\""))
}

fn installed(item: &Value) -> Result<Installed, String> {
  let record = item
    .get("record")
    .filter(|record| record.is_object())
    .ok_or("a plugin has no \"record\" object")?;
  Ok(Installed {
    name: text(record, "id")?,
    version: text(record, "version")?,
    enabled: record
      .get("enabled")
      .and_then(Value::as_bool)
      .unwrap_or(true),
    source: record
      .pointer("/source/path")
      .and_then(Value::as_str)
      .map(str::to_owned),
  })
}

/// Whether a `muse plugins inspect --json` payload has a runtime capability
/// (a hook, an agent definition) still awaiting approval.
pub(crate) fn pending(stdout: &str) -> Result<bool, String> {
  let value: Value = serde_json::from_str(stdout).map_err(|err| err.to_string())?;
  let capabilities = value
    .get("runtime_capabilities")
    .and_then(Value::as_array)
    .ok_or("expected an object with a \"runtime_capabilities\" array")?;
  let mut waiting = false;
  for capability in capabilities {
    let status = capability
      .get("status")
      .and_then(Value::as_str)
      .ok_or("a runtime capability has no string \"status\"")?;
    waiting |= PENDING.contains(&status);
  }
  Ok(waiting)
}

/// The source of the marketplace `name` in a `muse plugins marketplace list
/// --json` payload; `None` when it is not registered.
pub(crate) fn marketplace(stdout: &str, name: &str) -> Result<Option<Source>, String> {
  let value: Value = serde_json::from_str(stdout).map_err(|err| err.to_string())?;
  let Some(source) = registered_source(&value, name, "source")? else {
    return Ok(None);
  };
  Ok(Some(Source {
    kind: text(source, "kind")?,
    path: text(source, "path")?,
  }))
}

/// The `key` object of the marketplace `name` in a `{"marketplaces": [...]}`
/// listing, which Muse and Codex both print; `None` when it is not registered.
pub(crate) fn registered_source<'v>(
  value: &'v Value,
  name: &str,
  key: &str,
) -> Result<Option<&'v Value>, String> {
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
  entry
    .get(key)
    .map(Some)
    .ok_or(format!("marketplace {name} has no \"{key}\""))
}

/// `muse plugins marketplace list --json`.
pub(crate) fn marketplace_list() -> Vec<String> {
  argv(&["plugins", "marketplace", "list", "--json"])
}

/// Registers `dir` as the toolu marketplace.
pub(crate) fn marketplace_add(dir: &str) -> Vec<String> {
  argv(&["plugins", "marketplace", "add", MARKETPLACE, dir, "--json"])
}

/// Refreshes the toolu marketplace's snapshot from its directory.
pub(crate) fn marketplace_update() -> Vec<String> {
  argv(&["plugins", "marketplace", "update", MARKETPLACE, "--json"])
}

/// `muse <words…>`.
pub(crate) fn argv(words: &[&str]) -> Vec<String> {
  std::iter::once("muse")
    .chain(words.iter().copied())
    .map(str::to_owned)
    .collect()
}

#[cfg(test)]
#[path = "tests/muse_test.rs"]
mod tests;
