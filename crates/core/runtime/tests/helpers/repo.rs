//! The repository's shared fixtures: `{ "version": 1, "cases": [...] }` files.

use std::path::PathBuf;

use serde_json::Value;

/// A helper's result; the `#[test]` functions unwrap it.
pub(crate) type Res<T> = Result<T, String>;

/// `rel` under the repository root.
pub(crate) fn path(rel: &str) -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .join("../../..")
    .join(rel)
}

/// The `cases` of the case file `rel`.
pub(crate) fn cases(rel: &str) -> Res<Vec<Value>> {
  let text = std::fs::read_to_string(path(rel)).map_err(|err| format!("{rel}: {err}"))?;
  let doc: Value = serde_json::from_str(&text).map_err(|err| format!("{rel}: {err}"))?;
  if doc.get("version") != Some(&Value::from(1)) {
    return Err(format!("{rel}: not a version 1 case file"));
  }
  let cases = doc.get("cases").and_then(Value::as_array);
  cases
    .cloned()
    .ok_or_else(|| format!("{rel}: no cases array"))
}
