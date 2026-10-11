//! Test support: the recorded pr-babysit fixtures under `fixtures/pr-babysit`.

use std::path::{Path, PathBuf};

use serde_json::Value;

/// `fixtures/pr-babysit/<name>` in this checkout.
pub(crate) fn path(name: &str) -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../../fixtures/pr-babysit")
    .join(name)
}

/// The fixture `<name>` parsed as JSON.
pub(crate) fn json(name: &str) -> Value {
  let text = std::fs::read_to_string(path(name)).unwrap_or_default();
  serde_json::from_str(&text).unwrap_or(Value::Null)
}

/// The fixture `<name>` as text.
pub(crate) fn text(name: &str) -> String {
  std::fs::read_to_string(path(name)).unwrap_or_default()
}

#[cfg(test)]
#[path = "tests/fixture_test.rs"]
mod tests;
