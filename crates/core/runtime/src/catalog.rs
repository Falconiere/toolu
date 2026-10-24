//! The plugin catalog this binary was built with: the repository's
//! `.claude-plugin/marketplace.json`. `toolu plugins` validates and walks it;
//! `toolu doctor` reads only its plugin names, to tell a plugin the
//! marketplace retired from one it still ships (#615).

use serde_json::Value;

/// The marketplace manifest text, embedded at build time.
pub const BUILT_IN: &str = include_str!("../../../../.claude-plugin/marketplace.json");

/// The `plugins[].name` strings of manifest `text`, in manifest order; empty
/// when `text` is not JSON or has no `plugins` array.
pub fn plugin_names(text: &str) -> Vec<String> {
  let Ok(value) = serde_json::from_str::<Value>(text) else {
    return Vec::new();
  };
  value
    .get("plugins")
    .and_then(Value::as_array)
    .map(|plugins| {
      plugins
        .iter()
        .filter_map(|plugin| plugin.get("name").and_then(Value::as_str))
        .map(str::to_owned)
        .collect()
    })
    .unwrap_or_default()
}

#[cfg(test)]
#[path = "tests/catalog_test.rs"]
mod tests;
