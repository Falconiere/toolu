//! Salvage for a truncated structured review: the closed finding objects that
//! arrived before the cut, so a response that ran out of tokens still yields
//! what it finished, and the open tail is counted, never parsed.

use serde_json::Value;

/// Closed JSON objects inside a truncated `findings` array; an open trailing
/// object counts once and never becomes a finding.
pub(super) fn salvage(content: &str) -> Option<(Vec<Value>, usize)> {
  let array = array_body(content)?;
  let mut values = Vec::new();
  let mut start = None;
  let mut depth = 0_u32;
  let mut in_string = false;
  let mut escape = false;
  let mut dropped = 0;
  for (index, ch) in array.char_indices() {
    if in_string {
      if escape {
        escape = false;
      } else if ch == '\\' {
        escape = true;
      } else if ch == '"' {
        in_string = false;
      }
      continue;
    }
    match ch {
      '"' => in_string = true,
      '{' => {
        if depth == 0 {
          start = Some(index);
        }
        depth += 1;
      }
      '}' if depth > 0 => {
        depth -= 1;
        if depth == 0 {
          let parsed = closed_object(array, start?, index + 1);
          dropped += usize::from(parsed.is_none());
          values.extend(parsed);
          start = None;
        }
      }
      ']' if depth == 0 => break,
      _ => {}
    }
  }
  Some((values, dropped + usize::from(depth > 0)))
}

fn array_body(content: &str) -> Option<&str> {
  if !content.trim_start().starts_with('{') {
    return None;
  }
  let (_, after_key) = content.split_once("\"findings\"")?;
  let (_, after_colon) = after_key.split_once(':')?;
  after_colon.trim_start().strip_prefix('[')
}

fn closed_object(array: &str, start: usize, end: usize) -> Option<Value> {
  serde_json::from_str(array.get(start..end)?).ok()
}

#[cfg(test)]
#[path = "tests/salvage_test.rs"]
mod tests;
