//! toolu's native `session-start` hook: so far only the runtime diagnostic line.

use std::path::Path;

use serde_json::Value;

/// `toolu runtime: native <version> at <path>` when the payload's `source` is
/// `startup` or `resume`; nothing for another source or an unreadable payload.
pub(crate) fn diagnostic(payload: &str, version: &str, exe: Option<&Path>) -> Option<String> {
  let source = serde_json::from_str::<Value>(payload).ok()?;
  let source = source.get("source").and_then(Value::as_str)?;
  if source != "startup" && source != "resume" {
    return None;
  }
  let at = exe.map_or_else(
    || "an unknown path".to_owned(),
    |exe| exe.display().to_string(),
  );
  Some(format!("toolu runtime: native {version} at {at}"))
}

#[cfg(test)]
#[path = "tests/session_start_test.rs"]
mod tests;
