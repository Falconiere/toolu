//! Grok Build's machine-readable installed-plugin list.

use serde_json::Value;

use super::adapter::Installed;

/// Read the version and marketplace Grok records for each installation.
pub(crate) fn parse(text: &str) -> Result<Vec<Installed>, String> {
  let entries: Vec<Value> = serde_json::from_str(text).map_err(|err| err.to_string())?;
  entries.iter().map(entry).collect()
}

fn entry(value: &Value) -> Result<Installed, String> {
  let field = |key: &str| value.get(key).and_then(Value::as_str);
  Ok(Installed {
    name: field("name")
      .ok_or("Grok plugin entry has no name")?
      .to_owned(),
    version: field("version").unwrap_or("?").to_owned(),
    enabled: true,
    source: field("marketplace").map(str::to_owned),
  })
}

/// Whether Grok has one official source for its toolu marketplace.
pub(crate) fn official_marketplace(text: &str) -> Result<Option<bool>, String> {
  let rows: Vec<Value> = serde_json::from_str(text).map_err(|err| err.to_string())?;
  let mut sources = rows
    .iter()
    .filter(|row| row["name"] == super::adapter::MARKETPLACE);
  Ok(sources.next().map(|row| {
    sources.next().is_none()
      && row.pointer("/source/url").and_then(Value::as_str) == Some(super::checkout::ORIGIN)
      && row.pointer("/source/branch").is_none_or(Value::is_null)
  }))
}

#[cfg(test)]
#[path = "tests/grok_test.rs"]
mod tests;
