//! The repository's shell fixtures, read from the checkout.

use std::path::PathBuf;

use serde_json::Value;

/// `rel` under the repository root.
pub(crate) fn path(rel: &str) -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .join("../../..")
    .join(rel)
}

/// The JSON document `rel`.
pub(crate) fn json(rel: &str) -> Value {
  let text = std::fs::read_to_string(path(rel)).unwrap_or_else(|err| panic!("{rel}: {err}"));
  serde_json::from_str(&text).unwrap_or_else(|err| panic!("{rel}: {err}"))
}

/// The `cases` array of the fixture `rel`.
pub(crate) fn cases(rel: &str) -> Vec<Value> {
  let doc = json(rel);
  doc["cases"]
    .as_array()
    .cloned()
    .unwrap_or_else(|| panic!("{rel}: no cases"))
}

/// The string field `key` of `case`.
pub(crate) fn text<'a>(case: &'a Value, key: &str) -> &'a str {
  case[key]
    .as_str()
    .unwrap_or_else(|| panic!("no {key} in {case}"))
}
