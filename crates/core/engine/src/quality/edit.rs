//! File identification and worktree facts for one post-edit quality event.

use std::path::{Component, Path, PathBuf};

use serde_json::{Map, Value};
use toolu_protocol::normalized::NormalizedEvent;
use toolu_runtime::invocation::current_dir;
use toolu_runtime::json::jq_text;
use toolu_runtime::registry::rule::RuleContext;
use toolu_state::edit_records::{EditOperation, EditRecord};

/// The hook's original path, its absolute path, and whether it was removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditedFile {
  /// Gate key and message path, exactly as the hook named it.
  pub path: String,
  /// Filesystem path resolved against the hook working directory.
  pub absolute: PathBuf,
  /// A deleted file or move source, which needs only a gate clear.
  pub removed: bool,
}

/// jq's `a // b`: the first value that is neither null nor false.
fn input_field(input: &Map<String, Value>, keys: &[&str]) -> String {
  for key in keys {
    let Some(value) = input
      .get(*key)
      .filter(|value| !value.is_null() && **value != false)
    else {
      continue;
    };
    let text = value
      .as_str()
      .map_or_else(|| jq_text(&value.into(), true), str::to_owned);
    return text.trim_end_matches('\n').to_owned();
  }
  String::new()
}

fn field(ctx: &RuleContext<'_>, input: &Map<String, Value>, key: &str) -> String {
  let split = ctx.edit.and_then(|edit| match key {
    "MOVED_TO" => Some(edit.moved_to),
    _ => None,
  });
  let exported = if ctx.edit.is_some() {
    split
  } else {
    ctx.env.get(&format!("TOOLU_EDIT_{key}"))
  };
  exported.filter(|value| !value.is_empty()).map_or_else(
    || {
      input_field(
        input,
        &[&format!("toolu_edit_{}", key.to_ascii_lowercase())],
      )
    },
    str::to_owned,
  )
}

fn absolute(cwd: &Path, path: &str) -> PathBuf {
  let joined = cwd.join(path);
  let mut out = PathBuf::new();
  for component in joined.components() {
    match component {
      Component::ParentDir => {
        out.pop();
      }
      Component::CurDir => {}
      other @ (Component::Prefix(_) | Component::RootDir | Component::Normal(_)) => out.push(other),
    }
  }
  out
}

/// Resolve one normalized edit record for the shared batch runner.
pub fn edited_record(record: &EditRecord, ctx: &RuleContext<'_>) -> Option<EditedFile> {
  let cwd = ctx.cwd.unwrap_or(ctx.project_root);
  if record.path.is_empty() {
    return None;
  }
  Some(EditedFile {
    path: record.path.clone(),
    absolute: absolute(cwd, &record.path),
    removed: record.operation == EditOperation::Delete || record.moved_to.is_some(),
  })
}

/// Identify the edited file of a post-tool event, if one was named.
pub fn edited_file(event: &NormalizedEvent, ctx: &RuleContext<'_>) -> Option<EditedFile> {
  let tool = event.tool()?;
  let from_input = if matches!(tool.name.as_str(), "Write" | "Edit" | "MultiEdit") {
    input_field(&tool.input, &["path", "file_path", "target_file"])
  } else {
    String::new()
  };
  let path = ctx.env.get("CLAUDE_FILE_PATHS").unwrap_or(&from_input);
  if path.is_empty() {
    return None;
  }
  let removed = ctx
    .edit
    .is_some_and(|edit| edit.operation == toolu_runtime::registry::rule::EditOperation::Delete)
    || (ctx.edit.is_none() && field(ctx, &tool.input, "OPERATION") == "delete")
    || !field(ctx, &tool.input, "MOVED_TO").is_empty();
  let cwd = ctx
    .cwd
    .map(Path::to_path_buf)
    .or_else(|| current_dir().ok())?;
  Some(EditedFile {
    path: path.to_owned(),
    absolute: absolute(&cwd, path),
    removed,
  })
}

/// Whether `file` is a regular file; symlinks are followed.
pub fn is_regular_file(file: &EditedFile) -> bool {
  std::fs::metadata(&file.absolute).is_ok_and(|meta| meta.is_file())
}

/// Whether `file` lives under a linked Git worktree.
pub fn in_linked_worktree(file: &EditedFile, ctx: &RuleContext<'_>) -> bool {
  let dir = file.absolute.parent().unwrap_or(&file.absolute);
  toolu_state::git::linked_worktree(ctx.env, dir)
}

#[cfg(test)]
#[path = "tests/edit_test.rs"]
mod tests;
