//! What the post-tool gates read from the host's payload (`tool-exit.ts`),
//! exactly as `gate-status.sh` and `push-waiver.sh` read it with `jq -r`: the
//! command, the reported exit status and the interrupt flag. Each value is the
//! text bash would hold in its variable.

use serde_json::{Map, Value};
use toolu_runtime::json::jq_text;
use toolu_runtime::json::ordered::Ordered;

/// A jq type error: the bash caller's `|| echo "<fallback>"` path.
struct JqError;

/// jq `.key`: null through null and for a missing key, an error on anything
/// but an object.
fn index<'v>(value: Option<&'v Value>, key: &str) -> Result<Option<&'v Value>, JqError> {
  match value {
    None | Some(Value::Null) => Ok(None),
    Some(Value::Object(object)) => Ok(object.get(key)),
    Some(Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Array(_)) => Err(JqError),
  }
}

/// `<path> // <path> // …`: the first value that is neither null nor false,
/// `None` when every path falls through, an error when a path raises.
fn first_of<'v>(doc: &'v Value, paths: &[&[&str]]) -> Result<Option<&'v Value>, JqError> {
  for path in paths {
    let value = path.iter().try_fold(Some(doc), |at, key| index(at, key))?;
    if !matches!(value, None | Some(Value::Null | Value::Bool(false))) {
      return Ok(value);
    }
  }
  Ok(None)
}

/// One value as `jq -r` prints it and `$(…)` keeps it: strings raw, everything
/// else as pretty JSON, trailing newlines stripped.
fn printed(value: &Value) -> String {
  let text = match value {
    Value::String(text) => text.clone(),
    Value::Null | Value::Bool(_) | Value::Number(_) | Value::Array(_) | Value::Object(_) => {
      jq_text(&Ordered::from(value), true)
    }
  };
  text.trim_end_matches('\n').to_owned()
}

/// `$(jq -r '<paths> // <fallback>' || echo "")`: `fallback` when every path
/// falls through, "" when jq raises.
fn read(doc: &Value, paths: &[&[&str]], fallback: &str) -> String {
  match first_of(doc, paths) {
    Ok(Some(value)) => printed(value),
    Ok(None) => fallback.to_owned(),
    Err(JqError) => String::new(),
  }
}

/// The payload as one jq document.
fn document(payload: &Map<String, Value>) -> Value {
  Value::Object(payload.clone())
}

/// `.tool_input.command // ""`.
pub(crate) fn tool_command(payload: &Map<String, Value>) -> String {
  read(&document(payload), &[&["tool_input", "command"]], "")
}

const EXIT_PATHS: [&[&str]; 3] = [
  &["tool_response", "metadata", "exit_code"],
  &["tool_response", "exit_code"],
  &["tool_output", "exit_code"],
];

/// The reported exit status: `tool_response.metadata.exit_code`, then
/// `tool_response.exit_code`, then `tool_output.exit_code`, and when that is
/// empty or the text `null`, `exitCode`/`exit_code` inside `tool_output` (a
/// JSON string or object). "" when the host reported none.
pub(crate) fn tool_exit_status(payload: &Map<String, Value>) -> String {
  let doc = document(payload);
  let status = read(&doc, &EXIT_PATHS, "");
  if !status.is_empty() && status != "null" {
    return status;
  }
  let output = read(&doc, &[&["tool_output"]], "");
  if output.is_empty() {
    return status;
  }
  match serde_json::from_str::<Value>(&output) {
    Ok(inner) => read(&inner, &[&["exitCode"], &["exit_code"]], ""),
    Err(_) => String::new(),
  }
}

/// `.tool_response.interrupted // false` printed as `true`.
pub(crate) fn tool_interrupted(payload: &Map<String, Value>) -> bool {
  read(
    &document(payload),
    &[&["tool_response", "interrupted"]],
    "false",
  ) == "true"
}

#[cfg(test)]
#[path = "tests/tool_exit_test.rs"]
mod tests;
