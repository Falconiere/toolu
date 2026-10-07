//! The exact advisory a walk prints, as `jq -n` pretty-prints its merge.

use serde_json::Value;

/// `{"hookSpecificOutput":{"hookEventName","additionalContext"},"systemMessage"}`
/// with two-space indents and a newline, either part left out when `None`.
pub(crate) fn merged(event: &str, context: Option<&str>, message: Option<&str>) -> String {
  let quote = |text: &str| Value::from(text).to_string();
  let mut parts = Vec::new();
  if let Some(context) = context {
    parts.push(format!(
      "  \"hookSpecificOutput\": {{\n    \"hookEventName\": {},\n    \"additionalContext\": {}\n  }}",
      quote(event),
      quote(context)
    ));
  }
  if let Some(message) = message {
    parts.push(format!("  \"systemMessage\": {}", quote(message)));
  }
  format!("{{\n{}\n}}\n", parts.join(",\n"))
}
