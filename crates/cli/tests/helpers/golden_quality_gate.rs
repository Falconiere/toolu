//! The gate file with its top-level `violations` rebuilt in entry-key order.
//!
//! The aggregate is ordered by second-resolution `updatedAt`, so entries
//! written across a second boundary swap (#259). The text is left as is unless
//! the aggregate is exactly a reordering of the entries' violations; then only
//! that one line changes, so every other byte stays as the hook wrote it.

use serde_json::Value;

/// The line that opens the top-level `violations` of the pretty-printed file.
const AGGREGATE: &str = "  \"violations\": ";

fn utf16_len(text: &str) -> usize {
  text.encode_utf16().count()
}

/// The entries' violations joined in key order, when the aggregate is exactly a reordering.
fn rebuilt(doc: &Value) -> Option<String> {
  let entries = doc.get("entries")?.as_object()?;
  let aggregate = doc.get("violations")?.as_str()?;
  let parts: Vec<&str> = entries
    .values()
    .map(|entry| {
      entry
        .get("violations")
        .and_then(Value::as_str)
        .unwrap_or("")
    })
    .collect();
  let joined = parts.concat();
  let reordering =
    utf16_len(aggregate) == utf16_len(&joined) && parts.iter().all(|part| aggregate.contains(part));
  reordering.then_some(joined)
}

/// `text` with the aggregate in entry-key order when it is only reordered.
pub(crate) fn canonical_gate(text: &str) -> String {
  let Ok(doc) = serde_json::from_str::<Value>(text) else {
    return text.to_owned();
  };
  let Some(joined) = rebuilt(&doc) else {
    return text.to_owned();
  };
  let Ok(quoted) = serde_json::to_string(&joined) else {
    return text.to_owned();
  };
  text
    .split_inclusive('\n')
    .map(|line| match line.strip_prefix(AGGREGATE) {
      Some(rest) => {
        let comma = if rest.trim_end().ends_with(',') {
          ","
        } else {
          ""
        };
        let newline = if line.ends_with('\n') { "\n" } else { "" };
        format!("{AGGREGATE}{quoted}{comma}{newline}")
      }
      None => line.to_owned(),
    })
    .collect()
}
