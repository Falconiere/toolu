//! Reading `docs/cli/commands.json`, the command tree `toolu commands --json`
//! exports: shared by `check-cli-compat` and `check-markdown-cli`.

use serde_json::Value;

/// The array under `key`, or none.
pub(crate) fn list<'a>(node: &'a Value, key: &str) -> &'a [Value] {
  node
    .get(key)
    .and_then(Value::as_array)
    .map_or(&[], Vec::as_slice)
}

/// The string under `key`, or empty.
pub(crate) fn text<'a>(node: &'a Value, key: &str) -> &'a str {
  node.get(key).and_then(Value::as_str).unwrap_or_default()
}

/// Whether `key` is `true`.
pub(crate) fn flag_set(node: &Value, key: &str) -> bool {
  node.get(key) == Some(&Value::Bool(true))
}

#[cfg(test)]
#[path = "tests/command_tree_test.rs"]
mod tests;
