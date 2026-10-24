//! Cursor runs a plugin's Claude-format `hooks/hooks.json` and maps each event
//! to its camelCase name (#589). A hook on an event Cursor lacks would load and
//! never fire, so every event a `hooks.json` names must have a Cursor name.

use std::path::Path;

use serde_json::Value;
use toolu_protocol::host::Host;
use toolu_protocol::native::{canonical_event, native_event};

use crate::check_hooks::Finding;

/// The events of `hooks_json` that Cursor has no name for, or that no host knows.
fn unmapped(hooks_json: &Value) -> Vec<(String, String)> {
  let Some(events) = hooks_json.get("hooks").and_then(Value::as_object) else {
    return Vec::new();
  };
  events
    .keys()
    .filter_map(|event| {
      let Some(canonical) = canonical_event(Host::Claude, event) else {
        return Some((event.clone(), "is not a hook event of any host".to_owned()));
      };
      native_event(Host::Cursor, canonical).is_none().then(|| {
        (
          event.clone(),
          "has no Cursor event, so Cursor never runs it".to_owned(),
        )
      })
    })
    .collect()
}

/// Findings for `plugins/<plugin>/hooks/hooks.json`; none when it is absent or
/// unreadable (`hooks_entries` already reports those).
pub(crate) fn check(root: &Path, plugin: &str) -> Vec<Finding> {
  let file = format!("plugins/{plugin}/hooks/hooks.json");
  let Ok(text) = std::fs::read_to_string(root.join(&file)) else {
    return Vec::new();
  };
  let Ok(json) = serde_json::from_str::<Value>(&text) else {
    return Vec::new();
  };
  unmapped(&json)
    .into_iter()
    .map(|(event, problem)| Finding {
      file: file.clone(),
      at: event.clone(),
      problem: format!("{event} {problem}"),
      expected: None,
    })
    .collect()
}

#[cfg(test)]
#[path = "tests/hooks_cursor_test.rs"]
mod tests;
