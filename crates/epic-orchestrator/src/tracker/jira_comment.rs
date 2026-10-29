//! Match the single-paragraph Jira comment this tracker writes.

use serde_json::Value;

/// Compare a v2 string or v3 ADF body with the exact close note.
pub(super) fn comment_matches(row: &Value, note: &str) -> Result<bool, String> {
  let invalid = || "invalid Jira comments response".to_owned();
  let body = row.get("body").ok_or_else(invalid)?;
  if let Some(text) = body.as_str() {
    return Ok(text == note);
  }
  let doc = body
    .as_object()
    .filter(|doc| doc.get("type").and_then(Value::as_str) == Some("doc"))
    .ok_or_else(invalid)?;
  let content = doc
    .get("content")
    .and_then(Value::as_array)
    .ok_or_else(invalid)?;
  let [paragraph] = content.as_slice() else {
    return Ok(false);
  };
  let Some(nodes) = paragraph.get("content").and_then(Value::as_array) else {
    return Ok(false);
  };
  let [text] = nodes.as_slice() else {
    return Ok(false);
  };
  Ok(
    paragraph.get("type").and_then(Value::as_str) == Some("paragraph")
      && text.get("type").and_then(Value::as_str) == Some("text")
      && text.get("text").and_then(Value::as_str) == Some(note),
  )
}

#[cfg(test)]
#[path = "tests/jira_comment_test.rs"]
mod tests;
