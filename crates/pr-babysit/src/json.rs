//! JSON helpers with the shipped reducer's semantics: `null` and `false` fall
//! back like jq's `//`, a non-array reads as empty, and ids compare as jq
//! strings.

use serde_json::Value;

const EMPTY: &[Value] = &[];

/// An items document's `round`: 1 when absent or null, `None` unless a whole
/// number of at least 1, as TypeScript's `Number.isInteger(round ?? 1)`.
pub(crate) fn round(document: &Value) -> Option<u64> {
  match document.get("round") {
    None | Some(Value::Null) => Some(1),
    Some(round) => round
      .as_u64()
      .or_else(|| {
        round
          .as_f64()
          .and_then(|whole| whole.to_string().parse().ok())
      })
      .filter(|round| *round >= 1),
  }
}

/// `value` unless it is absent, `null` or `false`.
pub(crate) fn nil(value: Option<&Value>) -> Option<&Value> {
  value.filter(|value| !matches!(value, Value::Null | Value::Bool(false)))
}

/// `nil(value)`, or `fallback` as an owned value.
pub(crate) fn or(value: Option<&Value>, fallback: Value) -> Value {
  nil(value).cloned().unwrap_or(fallback)
}

/// The array at `value`, or an empty slice.
pub(crate) fn array(value: Option<&Value>) -> &[Value] {
  value.and_then(Value::as_array).map_or(EMPTY, Vec::as_slice)
}

/// A string as itself, anything else as its JSON text (jq `tostring`).
pub(crate) fn jq_string(value: &Value) -> String {
  value
    .as_str()
    .map_or_else(|| value.to_string(), ToOwned::to_owned)
}

/// The string at `key`, or `""`.
pub(crate) fn text<'v>(value: &'v Value, key: &str) -> &'v str {
  value.get(key).and_then(Value::as_str).unwrap_or("")
}

/// Whether the object at `map` has `key`.
pub(crate) fn has_key(map: Option<&Value>, key: &str) -> bool {
  map
    .and_then(Value::as_object)
    .is_some_and(|object| object.contains_key(key))
}

#[cfg(test)]
#[path = "tests/json_test.rs"]
mod tests;
