//! The jq semantics the ledger, verdict and waiver pipelines depend on
//! (`packages/toolu-core/src/ledger/ledger-jq.ts`). The bash libraries ran jq
//! over plain JSON, so a legacy or hand-edited file is accepted wherever jq
//! accepts it and fails exactly where jq raises a type error. `.key` and `.[]`
//! return [`JqError`] on the shapes jq rejects, `//` treats `false` like
//! `null`, and [`raw`] renders a value the way `jq -r` prints it.

use serde_json::Number;
use toolu_runtime::json::jq_text;
use toolu_runtime::json::ordered::Ordered;
use toolu_state::js_order::js_ordered;

/// A jq runtime error: the bash caller's `|| { echo "..."; return 2; }` path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JqError(pub String);

impl std::fmt::Display for JqError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(&self.0)
  }
}

impl std::error::Error for JqError {}

/// `null`, for lookups that find nothing.
pub static NULL: Ordered = Ordered::Null;

/// jq's `type`.
pub fn type_name(value: &Ordered) -> &'static str {
  match value {
    Ordered::Null => "null",
    Ordered::Bool(_) => "boolean",
    Ordered::Number(_) => "number",
    Ordered::String(_) => "string",
    Ordered::Array(_) => "array",
    Ordered::Object(_) => "object",
  }
}

/// jq `.[key]` for any key value: null through null, a missing key is null,
/// and anything but an object indexed by a string is an error.
///
/// # Errors
/// [`JqError`] for a non-null, non-object value, or a non-string key.
pub fn index<'a>(value: &'a Ordered, key: &Ordered) -> Result<&'a Ordered, JqError> {
  match (value, key) {
    (Ordered::Null, _) => Ok(&NULL),
    (Ordered::Object(_), Ordered::String(name)) => Ok(value.get(name).unwrap_or(&NULL)),
    _ => Err(JqError(format!(
      "Cannot index {} with {}",
      type_name(value),
      type_name(key)
    ))),
  }
}

/// jq `.key`.
///
/// # Errors
/// [`JqError`] for a non-null, non-object value.
pub fn get<'a>(value: &'a Ordered, key: &str) -> Result<&'a Ordered, JqError> {
  match value {
    Ordered::Null => Ok(&NULL),
    Ordered::Object(_) => Ok(value.get(key).unwrap_or(&NULL)),
    Ordered::Bool(_) | Ordered::Number(_) | Ordered::String(_) | Ordered::Array(_) => Err(JqError(
      format!("Cannot index {} with string", type_name(value)),
    )),
  }
}

/// jq `.[]`: array items or object values; anything else, null included, is an error.
///
/// # Errors
/// [`JqError`] for a scalar or null.
pub fn each(value: &Ordered) -> Result<Vec<&Ordered>, JqError> {
  match value {
    Ordered::Array(items) => Ok(items.iter().collect()),
    Ordered::Object(entries) => Ok(entries.iter().map(|(_, item)| item).collect()),
    Ordered::Null | Ordered::Bool(_) | Ordered::Number(_) | Ordered::String(_) => {
      Err(JqError(format!("Cannot iterate over {}", type_name(value))))
    }
  }
}

/// jq `.[]?`: like [`each`], but an un-iterable input yields nothing.
pub fn each_optional(value: &Ordered) -> Vec<&Ordered> {
  each(value).unwrap_or_default()
}

/// jq truthiness: everything but `null` and `false`.
pub fn truthy(value: &Ordered) -> bool {
  !matches!(value, Ordered::Null | Ordered::Bool(false))
}

/// jq `a // b`: `null` and `false` fall through to the fallback.
pub fn alt<'a>(value: &'a Ordered, fallback: &'a Ordered) -> &'a Ordered {
  if truthy(value) { value } else { fallback }
}

/// jq `==`: structural equality, numbers by value and objects by keys.
pub fn equals(a: &Ordered, b: &Ordered) -> bool {
  match (a, b) {
    (Ordered::Null, Ordered::Null) => true,
    (Ordered::Bool(x), Ordered::Bool(y)) => x == y,
    (Ordered::Number(x), Ordered::Number(y)) => x.as_f64() == y.as_f64(),
    (Ordered::String(x), Ordered::String(y)) => x == y,
    (Ordered::Array(x), Ordered::Array(y)) => {
      x.len() == y.len() && x.iter().zip(y).all(|(p, q)| equals(p, q))
    }
    (Ordered::Object(x), Ordered::Object(y)) => {
      x.len() == y.len()
        && x
          .iter()
          .all(|(key, item)| b.get(key).is_some_and(|other| equals(item, other)))
    }
    _ => false,
  }
}

/// `value == "text"`.
pub fn is_str(value: &Ordered, text: &str) -> bool {
  matches!(value, Ordered::String(own) if own == text)
}

/// jq `length`: 0 for null, the absolute value of a number, code points of a
/// string, items of an array and keys of an object.
///
/// # Errors
/// [`JqError`] for a boolean.
pub fn length(value: &Ordered) -> Result<f64, JqError> {
  match value {
    Ordered::Null => Ok(0.0),
    Ordered::Bool(_) => Err(JqError("boolean has no length".to_owned())),
    Ordered::Number(number) => Ok(number.as_f64().unwrap_or(0.0).abs()),
    Ordered::String(text) => Ok(count(text.chars().count())),
    Ordered::Array(items) => Ok(count(items.len())),
    Ordered::Object(entries) => Ok(count(entries.len())),
  }
}

/// A count as a JSON number.
pub fn count(n: usize) -> f64 {
  // Counts of in-memory items are far below 2^53, where f64 is exact.
  u32::try_from(n).map_or(f64::MAX, f64::from)
}

/// A JSON number; a non-finite value is `null`, as `JSON.stringify` prints it.
pub fn number(value: f64) -> Ordered {
  Number::from_f64(value).map_or(Ordered::Null, Ordered::Number)
}

/// jq `index($x)` for a string `$x`: the first position in an array or a
/// string, `null` when absent. On an object it is the first element of
/// `.[$x]`; a null input gives null.
///
/// # Errors
/// [`JqError`] for any other input, or an object whose `.[$x]` is not an array or null.
pub fn index_of(container: &Ordered, needle: &str) -> Result<Ordered, JqError> {
  match container {
    Ordered::Null => Ok(Ordered::Null),
    Ordered::Array(items) => Ok(
      items
        .iter()
        .position(|item| is_str(item, needle))
        .map_or(Ordered::Null, |at| number(count(at))),
    ),
    Ordered::String(text) => Ok(
      text
        .find(needle)
        .map_or(Ordered::Null, |at| number(count(at))),
    ),
    Ordered::Object(_) => match get(container, needle)? {
      Ordered::Null => Ok(Ordered::Null),
      Ordered::Array(items) => Ok(items.first().cloned().unwrap_or(Ordered::Null)),
      found @ (Ordered::Bool(_) | Ordered::Number(_) | Ordered::String(_) | Ordered::Object(_)) => {
        Err(JqError(format!(
          "Cannot index {} with number",
          type_name(found)
        )))
      }
    },
    Ordered::Bool(_) | Ordered::Number(_) => Err(JqError(format!(
      "Cannot index {} with string",
      type_name(container)
    ))),
  }
}

/// `select(X | index($x))`: jq truthiness of [`index_of`].
///
/// # Errors
/// As [`index_of`].
pub fn holds(container: &Ordered, needle: &str) -> Result<bool, JqError> {
  index_of(container, needle).map(|found| truthy(&found))
}

/// jq `tostring`: strings verbatim, everything else as compact JSON.
pub fn to_str(value: &Ordered) -> String {
  match value {
    Ordered::String(text) => text.clone(),
    Ordered::Null
    | Ordered::Bool(_)
    | Ordered::Number(_)
    | Ordered::Array(_)
    | Ordered::Object(_) => jq_text(value, false),
  }
}

/// One value as `jq -r` prints it: strings raw, everything else as pretty JSON.
pub fn raw(value: &Ordered) -> String {
  match value {
    Ordered::String(text) => text.clone(),
    Ordered::Null
    | Ordered::Bool(_)
    | Ordered::Number(_)
    | Ordered::Array(_)
    | Ordered::Object(_) => jq_text(value, true),
  }
}

/// jq `"a" + x` for a string on the left: only a string (or null) may follow.
///
/// # Errors
/// [`JqError`] for any part that is neither a string nor null.
pub fn concat(parts: &[&Ordered]) -> Result<String, JqError> {
  let mut out = String::new();
  for part in parts {
    match part {
      Ordered::Null => {}
      Ordered::String(text) => out.push_str(text),
      Ordered::Bool(_) | Ordered::Number(_) | Ordered::Array(_) | Ordered::Object(_) => {
        return Err(JqError(format!(
          "string and {} cannot be added",
          type_name(part)
        )));
      }
    }
  }
  Ok(out)
}

/// `JSON.parse(text)`, objects in JavaScript key order; `None` when `text` is
/// not one JSON document.
pub fn parse_json(text: &str) -> Option<Ordered> {
  Ordered::parse(text).ok().map(js_ordered)
}

/// A string value.
pub fn string(text: &str) -> Ordered {
  Ordered::String(text.to_owned())
}

/// jq `. + {key: value}` on an object: an existing key keeps its place.
///
/// # Errors
/// [`JqError`] for anything but an object or null.
pub fn assign(target: &Ordered, key: &str, value: Ordered) -> Result<Ordered, JqError> {
  let mut out = match target {
    Ordered::Null => Ordered::Object(Vec::new()),
    Ordered::Object(_) => target.clone(),
    Ordered::Bool(_) | Ordered::Number(_) | Ordered::String(_) | Ordered::Array(_) => {
      return Err(JqError(format!(
        "Cannot index {} with \"{key}\"",
        type_name(target)
      )));
    }
  };
  out.set(key, value);
  Ok(out)
}

#[cfg(test)]
#[path = "tests/jq_test.rs"]
mod tests;
