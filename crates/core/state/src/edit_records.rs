//! Edit payloads as records (`edit-records.ts`): `Edit`, `Write`, `MultiEdit`
//! and Codex `apply_patch` become one record per affected path. A malformed
//! payload yields no partial records; pre-tool callers treat that as a
//! fail-closed signal.

use serde_json::{Map, Value};
use toolu_runtime::json::jq_text;
use toolu_runtime::json::ordered::Ordered;

use crate::apply_patch::{apply_patch_records, path_valid};

/// The tools whose payloads are edits.
pub const EDIT_TOOLS: [&str; 4] = ["Edit", "Write", "MultiEdit", "apply_patch"];

/// What an edit does to its path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditOperation {
  /// A new file.
  Add,
  /// A changed file (a move's source too).
  Update,
  /// A removed file.
  Delete,
  /// A whole-file write.
  Write,
  /// A move's destination.
  Move,
}

impl EditOperation {
  /// The operation as the records spell it.
  pub fn name(self) -> &'static str {
    match self {
      EditOperation::Add => "add",
      EditOperation::Update => "update",
      EditOperation::Delete => "delete",
      EditOperation::Write => "write",
      EditOperation::Move => "move",
    }
  }
}

/// One affected path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditRecord {
  /// The path, as the payload names it.
  pub path: String,
  /// What happens to it.
  pub operation: EditOperation,
  /// A move source's destination.
  pub moved_to: Option<String>,
  /// A move destination's source.
  pub from: Option<String>,
}

/// A payload, normalized.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditRecords {
  /// Every affected path.
  Records(Vec<EditRecord>),
  /// The tool does not edit files.
  NotEdit,
  /// The payload cannot be read; fail closed.
  Malformed,
}

/// `EditRecordSchema`: a non-empty `path`, a known `operation`, optional
/// string `moved_to` and `from`, nothing else.
///
/// # Errors
/// The first offending key, as `<key>: <problem>`.
pub fn parse_edit_record(value: &Value) -> Result<EditRecord, String> {
  let object = value.as_object().ok_or("(root): expected object")?;
  if let Some(key) = object
    .keys()
    .find(|key| !["path", "operation", "moved_to", "from"].contains(&key.as_str()))
  {
    return Err(format!("(root): unrecognized key \"{key}\""));
  }
  let text = |key: &str| {
    let value = object.get(key).map(|value| {
      value
        .as_str()
        .ok_or_else(|| format!("{key}: expected string"))
    });
    value.transpose().map(|text| text.map(str::to_owned))
  };
  let path = text("path")?
    .filter(|path| !path.is_empty())
    .ok_or("path: expected a non-empty string")?;
  let operation = text("operation")?.unwrap_or_default();
  let operation = [
    EditOperation::Add,
    EditOperation::Update,
    EditOperation::Delete,
    EditOperation::Write,
    EditOperation::Move,
  ]
  .into_iter()
  .find(|known| known.name() == operation)
  .ok_or_else(|| format!("operation: unknown operation \"{operation}\""))?;
  Ok(EditRecord {
    path,
    operation,
    moved_to: text("moved_to")?,
    from: text("from")?,
  })
}

/// `toolu_is_edit_tool`.
pub fn is_edit_tool(tool: &str) -> bool {
  EDIT_TOOLS.contains(&tool)
}

/// A string read through `$(jq -r ...)`: trailing newlines stripped.
fn substituted(value: &str) -> &str {
  value.trim_end_matches('\n')
}

/// `.tool_input` as jq sees it.
enum Input<'a> {
  /// Absent or null: jq reads null.
  Null,
  /// An object.
  Object(&'a Map<String, Value>),
  /// jq would fail to index: a non-object payload or input.
  Unreadable,
}

fn tool_input(payload: &Value) -> Input<'_> {
  if payload.is_null() {
    return Input::Null;
  }
  let Some(payload) = payload.as_object() else {
    return Input::Unreadable;
  };
  match payload.get("tool_input") {
    None => Input::Null,
    Some(input) if input.is_null() => Input::Null,
    Some(input) => input.as_object().map_or(Input::Unreadable, Input::Object),
  }
}

/// jq's `a // b`: neither null nor false (absent reads as null).
fn truthy(value: Option<&Value>) -> Option<&Value> {
  value.filter(|value| !value.is_null() && **value != Value::Bool(false))
}

/// `.tool_input.file_path // .tool_input.path // .tool_input.target_file | strings`.
fn edit_path(input: Option<&Map<String, Value>>) -> Option<&str> {
  let field = |key: &str| input.and_then(|input| input.get(key));
  let value = truthy(field("file_path"))
    .or_else(|| truthy(field("path")))
    .or_else(|| field("target_file"));
  value.and_then(Value::as_str).map(substituted)
}

/// `toolu_normalize_edit_records INPUT TOOL`, over the parsed hook payload.
pub fn normalize_edit_records(payload: &Value, tool: &str) -> EditRecords {
  if !is_edit_tool(tool) {
    return EditRecords::NotEdit;
  }
  let input = match tool_input(payload) {
    Input::Null => None,
    Input::Object(input) => Some(input),
    Input::Unreadable => return EditRecords::Malformed,
  };
  if tool == "apply_patch" {
    let command = input
      .and_then(|input| input.get("command"))
      .and_then(Value::as_str);
    let records = command.and_then(|command| apply_patch_records(substituted(command)));
    return records.map_or(EditRecords::Malformed, EditRecords::Records);
  }
  match edit_path(input).filter(|path| path_valid(path)) {
    Some(path) => {
      let operation = if tool == "Write" {
        EditOperation::Write
      } else {
        EditOperation::Update
      };
      EditRecords::Records(vec![EditRecord {
        path: path.to_owned(),
        operation,
        moved_to: None,
        from: None,
      }])
    }
    None => EditRecords::Malformed,
  }
}

/// The records as the bash function printed them: one `jq -c` object per line.
pub fn format_edit_records(records: &[EditRecord]) -> String {
  let text = |value: &str| Ordered::String(value.to_owned());
  let mut out = String::new();
  for record in records {
    let mut fields = vec![
      ("path".to_owned(), text(&record.path)),
      ("operation".to_owned(), text(record.operation.name())),
    ];
    if let Some(to) = &record.moved_to {
      fields.push(("moved_to".to_owned(), text(to)));
    }
    if let Some(from) = &record.from {
      fields.push(("from".to_owned(), text(from)));
    }
    out.push_str(&jq_text(&Ordered::Object(fields), false));
    out.push('\n');
  }
  out
}

#[cfg(test)]
#[path = "tests/edit_records_test.rs"]
mod tests;
