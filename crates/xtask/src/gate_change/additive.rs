//! Whether a JSON document only grew: every value of `before` is still in
//! `after`. Registration data may grow next to product code; anything else
//! that changes it is a gate change.

use serde_json::Value;

/// Whether `after` keeps everything `before` has.
///
/// Objects keep every key additively; arrays of arrays grow position by
/// position (the layer table); any other array keeps each element somewhere.
pub(super) fn additive(before: &Value, after: &Value) -> bool {
  match (before, after) {
    (Value::Object(old), Value::Object(new)) => old
      .iter()
      .all(|(key, value)| new.get(key).is_some_and(|kept| additive(value, kept))),
    (Value::Array(old), Value::Array(new)) if old.iter().all(Value::is_array) => {
      new.len() >= old.len() && old.iter().zip(new).all(|(was, now)| additive(was, now))
    }
    (Value::Array(old), Value::Array(new)) => old.iter().all(|value| new.contains(value)),
    _ => before == after,
  }
}

#[cfg(test)]
#[path = "tests/additive_test.rs"]
mod tests;
