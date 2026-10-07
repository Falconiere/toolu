//! The gate file, the push-review document and the waiver files.

use std::path::Path;

use serde_json::{Value, json};
use toolu_state::gate_file::{GateRead, read_gate_file};
use toolu_state::gate_schema::{GateEntry, GateFile};
use toolu_state::git::branch_slug;

/// The gate check. `missing` when there is no project state directory.
pub(super) fn read(path: Option<&Path>) -> Value {
  let Some(path) = path else {
    return document("missing", None, &[]);
  };
  match read_gate_file(path) {
    GateRead::Missing => document("missing", None, &[]),
    GateRead::Malformed(reason) | GateRead::Unrecognized { reason, .. } => {
      document("invalid", Some(reason.as_str()), &[])
    }
    GateRead::Ok(GateFile::Passing { .. }) => document("passing", None, &[]),
    GateRead::Ok(GateFile::Failing {
      reason,
      source,
      file,
      violations,
      entries,
      updated_at,
    }) => {
      let slots = match &entries {
        Some(entries) => entries
          .iter()
          .map(|(file, entry)| slot(file, entry))
          .collect(),
        None => vec![one_slot(&file, &source, &reason, &violations, &updated_at)],
      };
      document("failing", Some(reason.as_str()), &slots)
    }
  }
}

/// Push review and waivers. Missing, with null waivers, when `branch` is empty.
pub(super) fn review(state: Option<&Path>, branch: &str) -> (Value, Value) {
  let empty = json!({"waiver": Value::Null, "pending": Value::Null});
  let Some(state) = state.filter(|_| !branch.is_empty()) else {
    return (review_value("", "missing", None, &Value::Null), empty);
  };
  let slug = branch_slug(branch);
  let dir = state.join("push-review");
  let path = dir.join(format!("{slug}.json"));
  let review = read_review(&path);
  let waivers = json!({
    "waiver": object_or_null(&dir.join(format!("{slug}.waiver.json"))),
    "pending": object_or_null(&dir.join(format!("{slug}.pending-waiver.json"))),
  });
  (review, waivers)
}

fn document(status: &str, reason: Option<&str>, entries: &[Value]) -> Value {
  json!({"status": status, "reason": reason, "entries": entries})
}

fn one_slot(file: &str, source: &str, reason: &str, violations: &str, updated_at: &str) -> Value {
  json!({
    "file": file,
    "source": source,
    "reason": reason,
    "violations": violations,
    "updatedAt": updated_at,
  })
}

fn slot(file: &str, entry: &GateEntry) -> Value {
  json!({
    "file": file,
    "source": entry.source,
    "reason": entry.reason,
    "violations": entry.violations,
    "updatedAt": entry.updated_at,
  })
}

fn read_review(path: &Path) -> Value {
  let shown = path.display().to_string();
  if !path.is_file() {
    return review_value(&shown, "missing", None, &Value::Null);
  }
  let Ok(text) = std::fs::read_to_string(path) else {
    return review_value(&shown, "invalid", Some("unreadable"), &Value::Null);
  };
  match serde_json::from_str::<Value>(&text) {
    Ok(document @ Value::Object(_)) => review_value(&shown, "recorded", None, &document),
    Ok(_) => review_value(&shown, "invalid", Some("not an object"), &Value::Null),
    Err(_) => review_value(&shown, "invalid", Some("not JSON"), &Value::Null),
  }
}

fn review_value(path: &str, state: &str, reason: Option<&str>, document: &Value) -> Value {
  json!({"path": path, "state": state, "reason": reason, "document": document})
}

fn object_or_null(path: &Path) -> Value {
  let Ok(text) = std::fs::read_to_string(path) else {
    return Value::Null;
  };
  match serde_json::from_str::<Value>(&text) {
    Ok(Value::Object(map)) => Value::Object(map),
    Ok(_) | Err(_) => Value::Null,
  }
}

#[cfg(test)]
#[path = "tests/gate_test.rs"]
mod tests;
