//! jq and C-locale helpers shared by the lifecycle hooks (`bash-compat.ts`).

use serde_json::Value;
use toolu_runtime::json::jq_text;
use toolu_runtime::json::ordered::Ordered;

/// `$(…)`: drop every trailing `\n`, and no other trailing character.
pub(crate) fn strip_trailing_newlines(text: &str) -> &str {
  text.trim_end_matches('\n')
}

/// `tr '[:upper:]' '[:lower:]'` in the C locale: ASCII letters only.
pub(crate) fn ascii_lower(text: &str) -> String {
  text
    .chars()
    .map(|c| {
      if c.is_ascii_uppercase() {
        c.to_ascii_lowercase()
      } else {
        c
      }
    })
    .collect()
}

/// Parse hook stdin. `None` is what jq sees on empty or invalid input.
pub(crate) fn parse_stdin(text: &str) -> Option<Value> {
  serde_json::from_str(text).ok()
}

/// `.key` of a JSON object. Arrays and scalars have no members.
pub(crate) fn member<'a>(doc: Option<&'a Value>, key: &str) -> Option<&'a Value> {
  doc
    .and_then(Value::as_object)
    .and_then(|object| object.get(key))
}

/// `jq -r` of one value: a string is raw, anything else is pretty JSON.
pub(crate) fn jq_raw(value: &Value) -> String {
  if let Value::String(text) = value {
    return text.clone();
  }
  jq_text(&Ordered::from(value), true)
}

/// jq `.key // fallback`: null, false, and absent take the fallback.
pub(crate) fn jq_alt(value: Option<&Value>, fallback: &str) -> String {
  match value {
    None | Some(Value::Null | Value::Bool(false)) => fallback.to_owned(),
    Some(other) => strip_trailing_newlines(&jq_raw(other)).to_owned(),
  }
}

/// First of `source`, `session_event`, `event` that jq would keep, else `startup`.
pub(crate) fn session_event(input: &str) -> String {
  let doc = parse_stdin(input);
  let picked = ["source", "session_event", "event"]
    .into_iter()
    .find_map(|key| kept(member(doc.as_ref(), key)));
  let event = jq_alt(picked, "startup");
  if event.is_empty() || event == "null" {
    "startup".to_owned()
  } else {
    event
  }
}

/// A member jq `//` would not replace: anything except absent, null, and false.
fn kept(value: Option<&Value>) -> Option<&Value> {
  match value {
    Some(value) if !value.is_null() && value.as_bool() != Some(false) => Some(value),
    _ => None,
  }
}

#[cfg(test)]
#[path = "tests/text_test.rs"]
mod tests;
