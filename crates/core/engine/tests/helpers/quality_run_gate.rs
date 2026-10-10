//! Read the source-owned entry texts from a real quality gate file.

use std::path::Path;

use serde_json::{Value, json};
use toolu_state::gate_file::{GateRead, read_gate_file};
use toolu_state::gate_schema::GateFile;

pub(crate) fn gate_entries(gate: &Path) -> Value {
  let GateRead::Ok(GateFile::Failing {
    entries: Some(entries),
    ..
  }) = read_gate_file(gate)
  else {
    return json!({});
  };
  Value::Object(
    entries
      .into_iter()
      .map(|(key, entry)| (key, Value::String(entry.violations)))
      .collect(),
  )
}
