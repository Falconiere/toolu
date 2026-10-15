//! A host-neutral tool event rendered as the stdin each host hands a command hook:
//! Claude Code's envelope, or Codex's, where a file edit arrives as `apply_patch`.

use std::path::Path;

use serde_json::{Map, Value, json};

use crate::golden_gates_model::{Host, Res};

const SESSION: &str = "harness-session";

/// A Bash call.
pub(crate) fn bash_fixture(command: &str) -> Value {
  json!({"kind": "tool", "event": "PreToolUse", "toolName": "Bash",
    "toolInput": {"command": command}})
}

fn text<'a>(input: &'a Map<String, Value>, key: &str) -> &'a str {
  input.get(key).and_then(Value::as_str).unwrap_or_default()
}

enum Patch {
  Add,
  Update,
}

fn patch_text(kind: &Patch, path: &str, lines: &[String]) -> String {
  let mut out = vec!["*** Begin Patch".to_owned()];
  match kind {
    Patch::Add => {
      out.push(format!("*** Add File: {path}"));
      out.extend(lines.iter().map(|line| format!("+{line}")));
    }
    Patch::Update => {
      out.push(format!("*** Update File: {path}"));
      out.push("@@".to_owned());
      out.extend(lines.iter().cloned());
    }
  }
  out.push("*** End Patch".to_owned());
  out.join("\n")
}

/// The path as the patch names it: relative to `cwd` when it lies inside, else as given.
fn patch_path(file: &str, cwd: &Path) -> String {
  Path::new(file).strip_prefix(cwd).map_or_else(
    |_| file.to_owned(),
    |rel| rel.to_string_lossy().into_owned(),
  )
}

fn prefixed(source: &str, mark: char) -> Vec<String> {
  source
    .split('\n')
    .map(|line| format!("{mark}{line}"))
    .collect()
}

/// Codex reports every `Edit` and `Write` as `apply_patch`.
fn codex_tool(name: &str, input: &Map<String, Value>, cwd: &Path) -> (String, Value) {
  let path = patch_path(text(input, "file_path"), cwd);
  match name {
    "Edit" => {
      let mut lines = prefixed(text(input, "old_string"), '-');
      lines.extend(prefixed(text(input, "new_string"), '+'));
      let command = patch_text(&Patch::Update, &path, &lines);
      ("apply_patch".to_owned(), json!({ "command": command }))
    }
    "Write" => {
      let lines: Vec<String> = text(input, "content")
        .split('\n')
        .map(str::to_owned)
        .collect();
      let command = patch_text(&Patch::Add, &path, &lines);
      ("apply_patch".to_owned(), json!({ "command": command }))
    }
    other => (other.to_owned(), Value::Object(input.clone())),
  }
}

/// The stdin JSON `host` hands a command hook for `fixture`, with the hook running in `cwd`.
pub(crate) fn render(host: Host, fixture: &Value, cwd: &Path) -> Res<Value> {
  let field = |key: &str| {
    fixture
      .get(key)
      .ok_or_else(|| format!("fixture lacks {key}"))
  };
  let event = field("event")?.as_str().ok_or("event must be a string")?;
  let name = field("toolName")?
    .as_str()
    .ok_or("toolName must be a string")?;
  let input = field("toolInput")?
    .as_object()
    .ok_or("toolInput must be an object")?;
  let mut out = Map::new();
  out.insert("session_id".to_owned(), json!(SESSION));
  out.insert("cwd".to_owned(), json!(cwd));
  out.insert("hook_event_name".to_owned(), json!(event));
  let (tool_name, tool_input) = match host {
    Host::Codex => codex_tool(name, input, cwd),
    Host::Claude => (name.to_owned(), Value::Object(input.clone())),
  };
  out.insert("tool_name".to_owned(), json!(tool_name));
  out.insert("tool_input".to_owned(), tool_input);
  if host == Host::Codex {
    out.insert("turn_id".to_owned(), json!("harness-turn"));
    out.insert("tool_use_id".to_owned(), json!("harness-call"));
  }
  if event == "PostToolUse" {
    let response = fixture.get("toolResponse").cloned().unwrap_or(Value::Null);
    out.insert("tool_response".to_owned(), response);
  }
  Ok(Value::Object(out))
}
