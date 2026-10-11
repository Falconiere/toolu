//! Claude's status-line payload, with the old jq field order and fallback behavior.

use serde_json::Value;

/// The text fields the renderer consumes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Payload {
  pub model: String,
  pub effort: String,
  pub cwd: String,
  pub ctx_size: String,
  pub ctx_used: String,
  pub ctx_pct: String,
}

fn scalar(value: &Value) -> String {
  match value {
    Value::String(text) => text.clone(),
    Value::Null | Value::Bool(_) | Value::Number(_) | Value::Array(_) | Value::Object(_) => {
      value.to_string()
    }
  }
}

/// jq `.key`: null passes through; indexing any other non-object aborts later fields.
fn at<'a>(value: &'a Value, path: &[&str]) -> Result<Option<&'a Value>, ()> {
  let mut current = value;
  for key in path {
    match current {
      Value::Null => return Ok(None),
      Value::Object(map) => current = map.get(*key).unwrap_or(&Value::Null),
      Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Array(_) => return Err(()),
    }
  }
  Ok(Some(current))
}

fn field(doc: &Value, paths: &[&[&str]], fallback: &str) -> Result<String, ()> {
  for path in paths {
    match at(doc, path)? {
      Some(Value::Null | Value::Bool(false)) | None => {}
      Some(value) => return Ok(scalar(value)),
    }
  }
  Ok(fallback.to_owned())
}

/// Parse the payload, retaining earlier fields when a later jq-style lookup fails.
pub(crate) fn read(stdin: Option<&str>) -> Payload {
  let mut out = Payload::default();
  let doc = stdin.and_then(|text| serde_json::from_str::<Value>(text).ok());
  if let Some(doc) = doc {
    let fields: [(&[&[&str]], &str); 6] = [
      (&[&["model", "display_name"]], "Claude"),
      (&[&["effort", "level"]], ""),
      (&[&["workspace", "current_dir"], &["cwd"]], ""),
      (&[&["context_window", "context_window_size"]], "0"),
      (&[&["context_window", "total_input_tokens"]], "0"),
      (&[&["context_window", "used_percentage"]], ""),
    ];
    let slots = [
      &mut out.model,
      &mut out.effort,
      &mut out.cwd,
      &mut out.ctx_size,
      &mut out.ctx_used,
      &mut out.ctx_pct,
    ];
    for (slot, (paths, fallback)) in slots.into_iter().zip(fields) {
      match field(&doc, paths, fallback) {
        Ok(value) => *slot = value,
        Err(()) => break,
      }
    }
  }
  if out.model.is_empty() {
    "Claude".clone_into(&mut out.model);
  }
  if out.ctx_size.is_empty() {
    "0".clone_into(&mut out.ctx_size);
  }
  if out.ctx_used.is_empty() {
    "0".clone_into(&mut out.ctx_used);
  }
  out
}

#[cfg(test)]
#[path = "tests/payload_test.rs"]
mod tests;
