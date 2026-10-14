//! One tool call of a case, dispatched through the real `toolu hook post-tools`
//! as the host would deliver it, and the project state it leaves.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use regex::Regex;
use serde_json::{Value, json};

use crate::Res;
use crate::cases::{Host, PatchFile, Recorded, Step};
use crate::gate::canonical_gate;
use crate::proc;
use crate::sandbox::Sandbox;
use crate::setup::hook_env;

/// The tool call that already ran: its name, input and response.
struct Call {
  name: String,
  input: Value,
  response: Value,
}

/// The text of an `apply_patch` call.
fn patch_text(files: &[PatchFile]) -> Res<String> {
  let mut out = vec!["*** Begin Patch".to_owned()];
  for file in files {
    match file.op.as_str() {
      "add" => {
        out.push(format!("*** Add File: {}", file.path));
        out.extend(file.lines.iter().map(|line| format!("+{line}")));
      }
      "update" => {
        out.push(format!("*** Update File: {}", file.path));
        out.push("@@".to_owned());
        out.extend(file.lines.iter().cloned());
      }
      "delete" => out.push(format!("*** Delete File: {}", file.path)),
      other => return Err(format!("unknown patch op: {other}").into()),
    }
  }
  out.push("*** End Patch".to_owned());
  Ok(out.join("\n"))
}

fn patch_call(command: &str) -> Call {
  Call {
    name: "apply_patch".to_owned(),
    input: json!({ "command": command }),
    response: json!("Done"),
  }
}

/// The call a step makes, as Claude Code reports it.
fn call_for(sb: &Sandbox, host: Host, step: &Step) -> Res<Call> {
  if let Some(files) = &step.patch {
    return Ok(patch_call(&patch_text(files)?));
  }
  if let Some(raw) = &step.raw_patch {
    return Ok(patch_call(raw));
  }
  let file = step.file.as_deref().unwrap_or("");
  let path = if step.relative {
    file.to_owned()
  } else {
    sb.path(file)?.display().to_string()
  };
  let tool = step.tool.as_deref().unwrap_or("Write");
  if tool == "Delete" && host == Host::Codex {
    let gone = PatchFile {
      op: "delete".to_owned(),
      path: file.to_owned(),
      lines: Vec::new(),
    };
    return Ok(patch_call(&patch_text(&[gone])?));
  }
  if tool == "Delete" {
    return Ok(Call {
      name: "Edit".to_owned(),
      input: json!({ "file_path": path }),
      response: json!({ "success": true }),
    });
  }
  let content = step.write.get(file).map_or("", String::as_str);
  let mut input = if tool == "Write" {
    json!({ "file_path": path, "content": content })
  } else {
    json!({ "file_path": path })
  };
  if let Some(fields) = input.as_object_mut() {
    fields.extend(
      step
        .input
        .iter()
        .map(|(key, value)| (key.clone(), value.clone())),
    );
  }
  Ok(Call {
    name: tool.to_owned(),
    input,
    response: json!({ "success": true }),
  })
}

fn text<'a>(input: &'a Value, key: &str) -> &'a str {
  input.get(key).and_then(Value::as_str).unwrap_or("")
}

/// A path inside `cwd` as relative; any other path as given.
fn patch_path(file: &str, cwd: &Path) -> String {
  Path::new(file)
    .strip_prefix(cwd)
    .map_or_else(|_| file.to_owned(), |rel| rel.display().to_string())
}

/// Codex reports every file edit as `apply_patch`.
fn codex_call(call: Call, cwd: &Path) -> Res<Call> {
  let path = patch_path(text(&call.input, "file_path"), cwd);
  let lines_of = |key: &str, mark: &str| -> Vec<String> {
    text(&call.input, key)
      .split('\n')
      .map(|line| format!("{mark}{line}"))
      .collect()
  };
  let file = match call.name.as_str() {
    "Edit" => {
      let mut lines = lines_of("old_string", "-");
      lines.extend(lines_of("new_string", "+"));
      PatchFile {
        op: "update".to_owned(),
        path,
        lines,
      }
    }
    "Write" => {
      let lines = text(&call.input, "content")
        .split('\n')
        .map(str::to_owned)
        .collect();
      PatchFile {
        op: "add".to_owned(),
        path,
        lines,
      }
    }
    _ => return Ok(call),
  };
  Ok(Call {
    response: call.response,
    ..patch_call(&patch_text(&[file])?)
  })
}

/// The `PostToolUse` payload `host` sends for `call`.
fn payload(sb: &Sandbox, host: Host, call: Call) -> Res<String> {
  let call = if host == Host::Codex {
    codex_call(call, &sb.project)?
  } else {
    call
  };
  let mut doc = json!({
    "session_id": "harness-session",
    "cwd": sb.project,
    "hook_event_name": "PostToolUse",
    "tool_name": call.name,
    "tool_input": call.input,
    "tool_response": call.response,
  });
  if host == Host::Codex
    && let Some(fields) = doc.as_object_mut()
  {
    fields.insert("turn_id".to_owned(), json!("harness-turn"));
    fields.insert("tool_use_id".to_owned(), json!("harness-call"));
  }
  Ok(doc.to_string())
}

fn files_under(dir: &Path, out: &mut Vec<PathBuf>) -> Res<()> {
  let entries = match std::fs::read_dir(dir) {
    Ok(entries) => entries,
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
    Err(err) => return Err(err.into()),
  };
  for entry in entries {
    let path = entry?.path();
    if std::fs::metadata(&path)?.is_dir() {
      files_under(&path, out)?;
    } else {
      out.push(path);
    }
  }
  Ok(())
}

/// Every file under the project's `.claude/` and `.codex/`, keyed relative to the
/// project, ISO timestamps as `<T>`.
fn project_state(sb: &Sandbox) -> Res<BTreeMap<String, String>> {
  let stamp = Regex::new(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z")?;
  let mut paths = Vec::new();
  for dir in [".claude", ".codex"] {
    files_under(&sb.project.join(dir), &mut paths)?;
  }
  let mut state = BTreeMap::new();
  for path in paths {
    let body = String::from_utf8_lossy(&std::fs::read(&path)?).into_owned();
    let rel = path.strip_prefix(&sb.project)?.display().to_string();
    state.insert(rel, stamp.replace_all(&body, "<T>").into_owned());
  }
  Ok(state)
}

/// Sandbox paths become `<ROOT>` so captures from different sandboxes compare.
fn normalise(text: &str, sb: &Sandbox) -> String {
  text.replace(&sb.root.display().to_string(), "<ROOT>")
}

/// Dispatch `step` through the post-tools hook and record what happened.
pub(crate) fn dispatch(
  sb: &Sandbox,
  host: Host,
  case_env: &BTreeMap<String, String>,
  step: &Step,
) -> Res<Recorded> {
  for (rel, body) in &step.write {
    sb.write(rel, body)?;
  }
  for rel in &step.remove {
    match std::fs::remove_file(sb.path(rel)?) {
      Err(err) if err.kind() != std::io::ErrorKind::NotFound => return Err(err.into()),
      Ok(()) | Err(_) => {}
    }
  }
  let mut extra = case_env.clone();
  extra.extend(
    step
      .env
      .iter()
      .map(|(key, value)| (key.clone(), value.clone())),
  );
  // A Claude deletion reaches the module as an Edit with this operation.
  if step.tool.as_deref() == Some("Delete") && host == Host::Claude {
    extra.insert("TOOLU_EDIT_OPERATION".to_owned(), "delete".to_owned());
  }
  let env = hook_env(sb, host, &extra);
  let stdin = payload(sb, host, call_for(sb, host, step)?)?;
  let args = ["hook", "post-tools", "--event", "PostToolUse"];
  let ran = proc::toolu(&args, &sb.project, &env, &stdin)?;
  let state = project_state(sb)?
    .into_iter()
    .map(|(key, body)| {
      let body = normalise(&body, sb);
      let body = if key.ends_with("quality-gate-status.json") {
        canonical_gate(&body)
      } else {
        body
      };
      (normalise(&key, sb), body)
    })
    .collect();
  Ok(Recorded {
    stdout: normalise(&ran.stdout, sb),
    stderr: normalise(&ran.stderr, sb),
    exit_code: ran.code,
    state,
  })
}
