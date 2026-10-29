//! Secret redaction for complete fields and bounded log pages.

use ring::digest::{Context, SHA256};
use serde_json::Value;

use super::epic::credential_key;
use super::secrets::{REDACTED, Secrets};

/// Replace every resolved secret substring in ordinary text.
pub fn redact_text(text: &str, secrets: &Secrets) -> String {
  secrets
    .values()
    .into_iter()
    .fold(text.to_owned(), |shown, value| {
      shown.replace(value, REDACTED)
    })
}

/// Change when any current or shadowed redaction value changes.
pub fn redaction_fingerprint(secrets: &Secrets) -> [u8; 32] {
  let mut hash = Context::new(&SHA256);
  for value in secrets.values() {
    hash.update(&value.len().to_le_bytes());
    hash.update(value.as_bytes());
  }
  let mut fingerprint = [0; 32];
  fingerprint.copy_from_slice(hash.finish().as_ref());
  fingerprint
}

/// Bytes of context needed on each side of a page to detect split secrets.
pub fn redaction_overlap(secrets: &Secrets) -> usize {
  secrets
    .values()
    .into_iter()
    .map(str::len)
    .max()
    .unwrap_or_default()
    .saturating_sub(1)
}

fn hide_match(hidden: &mut [bool], start: usize, end: usize, position: usize, length: usize) {
  let left = position.max(start);
  let right = position.saturating_add(length).min(end);
  if left < right {
    for flag in hidden.iter_mut().skip(left - start).take(right - left) {
      *flag = true;
    }
  }
}

/// Redact a byte page using surrounding bytes to catch values split at its edges.
pub fn redact_window(window: &[u8], start: usize, end: usize, secrets: &Secrets) -> String {
  let Some(page) = window.get(start..end) else {
    return String::new();
  };
  let mut hidden = vec![false; page.len()];
  for value in secrets.values() {
    let pattern = value.as_bytes();
    if pattern.is_empty() {
      continue;
    }
    for (position, found) in window.windows(pattern.len()).enumerate() {
      if found != pattern {
        continue;
      }
      hide_match(&mut hidden, start, end, position, pattern.len());
    }
  }
  let mut shown = String::new();
  let mut position = 0;
  while position < page.len() {
    let mask = hidden.get(position).copied().unwrap_or_default();
    let end = hidden
      .iter()
      .skip(position)
      .position(|flag| *flag != mask)
      .map_or(page.len(), |next| position + next);
    if mask {
      shown.push_str(REDACTED);
    } else {
      shown.push_str(&String::from_utf8_lossy(
        page.get(position..end).unwrap_or_default(),
      ));
    }
    position = end;
  }
  shown
}

/// Replace credential-named JSON fields and resolved secret substrings.
pub fn redact_json(value: &Value, secrets: &Secrets) -> Value {
  match value {
    Value::Object(map) => Value::Object(
      map
        .iter()
        .map(|(key, value)| {
          let shown = if credential_key(key) {
            Value::String(REDACTED.to_owned())
          } else {
            redact_json(value, secrets)
          };
          (key.clone(), shown)
        })
        .collect(),
    ),
    Value::Array(items) => Value::Array(
      items
        .iter()
        .map(|item| redact_json(item, secrets))
        .collect(),
    ),
    Value::String(text) => Value::String(redact_text(text, secrets)),
    Value::Null | Value::Bool(_) | Value::Number(_) => value.clone(),
  }
}

#[cfg(test)]
#[path = "tests/redaction_test.rs"]
mod tests;
