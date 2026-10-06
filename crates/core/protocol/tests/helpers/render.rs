//! Host stdin rendered from the fixtures' tool descriptors, as the TypeScript
//! harness renders it: `toStdin`, `bashFixture` and the patch helpers of
//! `tools/toolu-conformance/src/harness/fixtures.ts`, and the step rule of the
//! quality golden harnesses (`fixtureFor` in `plugins/*-quality/hooks/src/__tests__/golden-harness.ts`).
//! Tagged `{"$path": …}` and `{"$template": …}` values expand against fixed roots.

use serde_json::{Map, Value, json};
use toolu_protocol::host::Host;

use crate::repo::Res;

/// The sandbox project every payload runs in.
pub(crate) const PROJECT: &str = "/sandbox/project";

const TOKENS: [(&str, &str); 4] = [
  ("$PROJECT", PROJECT),
  ("$ROOT", "/sandbox"),
  ("$HOME", "/sandbox/home"),
  ("$HOST_STATE", "/sandbox/state"),
];

/// `value` with every tagged `$path` or `$template` replaced by its expanded string.
pub(crate) fn expand(value: &Value) -> Value {
  match value {
    Value::Array(items) => Value::Array(items.iter().map(expand).collect()),
    Value::Object(object) => match tagged(object) {
      Some(text) => Value::from(TOKENS.iter().fold(text.to_owned(), |out, (token, root)| {
        out.replace(token, root)
      })),
      None => Value::Object(
        object
          .iter()
          .map(|(key, item)| (key.clone(), expand(item)))
          .collect(),
      ),
    },
    Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => value.clone(),
  }
}

fn tagged(object: &Map<String, Value>) -> Option<&str> {
  if object.len() != 1 {
    return None;
  }
  let text = object.get("$path").or_else(|| object.get("$template"));
  text.and_then(Value::as_str)
}

/// `bashFixture(command)`.
pub(crate) fn bash(command: &str) -> Value {
  json!({ "kind": "tool", "event": "PreToolUse", "toolName": "Bash",
    "toolInput": { "command": command } })
}

fn ran(tool: &str, input: &Value, response: &Value) -> Value {
  json!({ "kind": "tool", "event": "PostToolUse", "toolName": tool, "toolInput": input,
    "toolResponse": response })
}

fn str_of<'a>(value: &'a Value, key: &str) -> &'a str {
  value.get(key).and_then(Value::as_str).unwrap_or("")
}

/// `patchText`: one `*** Begin Patch` block over `files` (`{op, path, lines?}`).
fn patch_text(files: &[Value]) -> String {
  let mut lines = vec!["*** Begin Patch".to_owned()];
  for file in files {
    let path = str_of(file, "path");
    let body = file.get("lines").and_then(Value::as_array);
    let body = body.into_iter().flatten().filter_map(Value::as_str);
    match str_of(file, "op") {
      "add" => {
        lines.push(format!("*** Add File: {path}"));
        lines.extend(body.map(|line| format!("+{line}")));
      }
      "update" => {
        lines.extend([format!("*** Update File: {path}"), "@@".to_owned()]);
        lines.extend(body.map(str::to_owned));
      }
      _ => lines.push(format!("*** Delete File: {path}")),
    }
  }
  lines.push("*** End Patch".to_owned());
  lines.join("\n")
}

/// A quality `steps` entry as the `PostToolUse` descriptor `fixtureFor` builds for `host`.
pub(crate) fn quality_step(step: &Value, host: Host) -> Value {
  if let Some(Value::Array(files)) = step.get("patch") {
    return ran(
      "apply_patch",
      &json!({ "command": patch_text(files) }),
      &json!("Done"),
    );
  }
  if let Some(raw) = step.get("rawPatch").and_then(Value::as_str) {
    return ran("apply_patch", &json!({ "command": raw }), &json!("Done"));
  }
  let file = str_of(step, "file");
  let relative = step.get("relative") == Some(&Value::Bool(true));
  let path = if relative {
    file.to_owned()
  } else {
    format!("{PROJECT}/{file}")
  };
  let tool = step.get("tool").and_then(Value::as_str).unwrap_or("Write");
  if tool == "Delete" && host == Host::Codex {
    let files = [json!({ "op": "delete", "path": file })];
    return ran(
      "apply_patch",
      &json!({ "command": patch_text(&files) }),
      &json!("Done"),
    );
  }
  if tool == "Delete" {
    return ran(
      "Edit",
      &json!({ "file_path": path }),
      &json!({ "success": true }),
    );
  }
  let mut input = Map::from_iter([("file_path".to_owned(), Value::from(path))]);
  if tool == "Write" {
    let content = step.get("write").and_then(|write| write.get(file));
    input.insert("content".to_owned(), content.cloned().unwrap_or(json!("")));
  }
  if let Some(Value::Object(extra)) = step.get("input") {
    input.extend(extra.clone());
  }
  ran(tool, &Value::Object(input), &json!({ "success": true }))
}

/// `codexTool`: Codex reports every file edit as `apply_patch`.
fn codex_tool(name: &str, input: &Value, cwd: &str) -> (String, Value) {
  let file = str_of(input, "file_path");
  let path = file.strip_prefix(&format!("{cwd}/")).unwrap_or(file);
  let lines = |key: &str, mark: char| -> Vec<Value> {
    str_of(input, key)
      .split('\n')
      .map(|line| Value::from(format!("{mark}{line}")))
      .collect()
  };
  let file = match name {
    "Edit" => {
      let mut body = lines("old_string", '-');
      body.extend(lines("new_string", '+'));
      json!({ "op": "update", "path": path, "lines": body })
    }
    "Write" => {
      let body: Vec<&str> = str_of(input, "content").split('\n').collect();
      json!({ "op": "add", "path": path, "lines": body })
    }
    _ => return (name.to_owned(), input.clone()),
  };
  let command = patch_text(&[file]);
  ("apply_patch".to_owned(), json!({ "command": command }))
}

/// A JSON object of `pairs`.
fn object<const N: usize>(pairs: [(&str, Value); N]) -> Map<String, Value> {
  pairs
    .into_iter()
    .map(|(key, value)| (key.to_owned(), value))
    .collect()
}

/// `toStdin(host, fixture, { cwd })` for a tool descriptor.
pub(crate) fn stdin(host: Host, fixture: &Value, cwd: &str) -> Res<Value> {
  let name = str_of(fixture, "toolName");
  let input = fixture
    .get("toolInput")
    .cloned()
    .unwrap_or_else(|| json!({}));
  let event = str_of(fixture, "event");
  if host == Host::Cursor {
    return cursor(fixture, name, &input, cwd);
  }
  let mut out = object([
    ("session_id", json!("harness-session")),
    ("cwd", json!(cwd)),
    ("hook_event_name", json!(event)),
  ]);
  let (name, input) = if host == Host::Codex {
    out.extend(object([
      ("turn_id", json!("harness-turn")),
      ("tool_use_id", json!("harness-call")),
    ]));
    codex_tool(name, &input, cwd)
  } else {
    (name.to_owned(), input)
  };
  out.extend(object([("tool_name", json!(name)), ("tool_input", input)]));
  if event == "PostToolUse" {
    let response = fixture.get("toolResponse").cloned().unwrap_or(Value::Null);
    out.insert("tool_response".to_owned(), response);
  }
  Ok(Value::Object(out))
}

/// `cursorStdin`: only `PreToolUse` is modelled.
fn cursor(fixture: &Value, name: &str, input: &Value, cwd: &str) -> Res<Value> {
  if str_of(fixture, "event") != "PreToolUse" {
    return Err("cursor: only PreToolUse is modelled".into());
  }
  let base = |hook: &str| {
    object([
      ("conversation_id", json!("harness-session")),
      ("hook_event_name", json!(hook)),
      ("workspace_roots", json!([cwd])),
    ])
  };
  let mut out;
  if name == "Bash" {
    out = base("beforeShellExecution");
    out.extend(object([
      ("command", json!(str_of(input, "command"))),
      ("cwd", json!(cwd)),
      ("sandbox", json!(false)),
    ]));
  } else if let Some(mcp) = fixture.get("mcp") {
    out = base("beforeMCPExecution");
    out.extend(object([
      ("tool_name", json!(str_of(mcp, "tool"))),
      ("tool_input", json!(input.to_string())),
      ("mcp_server_name", json!(str_of(mcp, "server"))),
    ]));
  } else {
    out = base("preToolUse");
    out.extend(object([
      ("tool_name", json!(name)),
      ("tool_input", input.clone()),
      ("tool_use_id", json!("harness-call")),
      ("cwd", json!(cwd)),
    ]));
  }
  Ok(Value::Object(out))
}
