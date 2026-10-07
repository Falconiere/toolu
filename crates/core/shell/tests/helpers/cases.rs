//! The cases of the shared shell fixtures, read from the checkout without a
//! panic: a failure is a `String` that the `#[test]` functions unwrap.

use std::path::PathBuf;

use serde_json::Value;

/// A helper's result; the `#[test]` functions unwrap it.
pub(crate) type Res<T> = Result<T, String>;

/// The `cases` of the fixture `rel`, under the repository root.
pub(crate) fn cases(rel: &str) -> Res<Vec<Value>> {
  let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .join("../../..")
    .join(rel);
  let text = std::fs::read_to_string(path).map_err(|err| format!("{rel}: {err}"))?;
  let doc: Value = serde_json::from_str(&text).map_err(|err| format!("{rel}: {err}"))?;
  let cases = doc.get("cases").and_then(Value::as_array);
  cases
    .cloned()
    .ok_or_else(|| format!("{rel}: no cases array"))
}

/// The string field `key` of `case`.
pub(crate) fn field<'a>(case: &'a Value, key: &str) -> Res<&'a str> {
  let value = case.get(key).and_then(Value::as_str);
  value.ok_or_else(|| format!("no {key} in {case}"))
}
