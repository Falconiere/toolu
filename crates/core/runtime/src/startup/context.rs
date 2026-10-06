//! `SessionStart` context output (`context.ts`): the `hookSpecificOutput` shape
//! Claude Code and Codex both accept, bounded to the length a host injects,
//! printed with jq's bytes.

use crate::json::jq_text;
use crate::json::ordered::Ordered;

/// Claude Code's ceiling on `additionalContext`, in UTF-16 units; longer text is cut.
pub const MAX_CONTEXT_CHARS: usize = 10_000;

/// `text` cut to `max` UTF-16 units, never splitting a surrogate pair.
fn bounded(text: &str, max: usize) -> &str {
  let mut units = 0;
  for (at, c) in text.char_indices() {
    units += c.len_utf16();
    if units > max {
      return text.get(..at).unwrap_or(text);
    }
  }
  text
}

/// `{"hookSpecificOutput":{"hookEventName":…,"additionalContext":…}}`, or
/// `None` when there is nothing to say.
pub fn session_context(event: &str, text: &str) -> Option<Ordered> {
  if text.is_empty() {
    return None;
  }
  let output = Ordered::Object(vec![
    (
      "hookEventName".to_owned(),
      Ordered::String(event.to_owned()),
    ),
    (
      "additionalContext".to_owned(),
      Ordered::String(bounded(text, MAX_CONTEXT_CHARS).to_owned()),
    ),
  ]);
  Some(Ordered::Object(vec![(
    "hookSpecificOutput".to_owned(),
    output,
  )]))
}

/// `value` as `jq -n` (pretty) or `jq -nc` (compact) prints it, newline-terminated.
pub fn render_hook_output(value: &Ordered, pretty: bool) -> String {
  jq_text(value, pretty) + "\n"
}

#[cfg(test)]
#[path = "tests/context_test.rs"]
mod tests;
