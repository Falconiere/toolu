//! The cases of `events.test.ts`, every type on the strict wire, and its refusals.

use serde_json::{Value, json};

use super::{NormalizedEvent, Session, Tool};
use crate::event::EventKind;
use crate::text::Text;

/// The session fields every event carries.
fn base(kind: &str) -> Value {
  json!({
    "type": kind,
    "sessionId": "s1",
    "cwd": "/repo",
    "projectRoot": "/repo",
    "worktree": "/repo",
  })
}

fn with(mut doc: Value, fields: &Value) -> Value {
  let extra = fields.as_object().unwrap();
  doc.as_object_mut().unwrap().extend(extra.clone());
  doc
}

fn tool(kind: &str) -> Value {
  with(
    base(kind),
    &json!({ "toolCallId": "c1", "toolName": "Edit", "toolInput": { "file_path": "/repo/a.ts" } }),
  )
}

fn parse(doc: Value) -> Result<NormalizedEvent, String> {
  serde_json::from_value(doc).map_err(|err| err.to_string())
}

/// One valid document of every type.
fn every_type() -> Vec<Value> {
  let mut docs: Vec<Value> = [
    "session/start",
    "session/resume",
    "session/clear",
    "session/unload",
    "pre_compact",
    "compaction",
  ]
  .into_iter()
  .map(base)
  .collect();
  docs.push(with(base("prompt"), &json!({ "prompt": "fix the bug" })));
  docs.push(with(
    base("permission/evaluate"),
    &json!({ "permission": "edit" }),
  ));
  docs.push(tool("tool/pre"));
  docs.push(with(
    tool("tool/post"),
    &json!({ "toolOutput": { "ok": true } }),
  ));
  docs.push(with(tool("shell/pre"), &json!({ "command": "ls" })));
  docs
}

#[test]
fn every_type_parses_and_names_its_kind() {
  let kinds: Vec<EventKind> = every_type()
    .into_iter()
    .map(|doc| parse(doc).unwrap().kind())
    .collect();
  assert_eq!(kinds.len(), EventKind::ALL.len());
  for kind in EventKind::ALL {
    assert!(kinds.contains(&kind), "{kind:?}");
  }
}

#[test]
fn every_type_round_trips_to_an_equal_value() {
  for doc in every_type() {
    let event = parse(doc).unwrap();
    let again: NormalizedEvent =
      serde_json::from_str(&serde_json::to_string(&event).unwrap()).unwrap();
    assert_eq!(again, event);
  }
}

#[test]
fn tool_pre_carries_its_session_and_tool() {
  let event = parse(tool("tool/pre")).unwrap();
  assert_eq!(event.kind(), EventKind::ToolPre);
  assert_eq!(event.session().session_id.as_str(), "s1");
  assert_eq!(event.session().worktree.as_str(), "/repo");
  let tool = event.tool().unwrap();
  assert_eq!(tool.call_id.as_str(), "c1");
  assert_eq!(tool.name.as_str(), "Edit");
  assert_eq!(tool.input.get("file_path"), Some(&json!("/repo/a.ts")));
}

#[test]
fn session_events_have_no_tool() {
  let event = parse(base("session/start")).unwrap();
  assert!(event.tool().is_none());
  assert_eq!(event.session().cwd.as_str(), "/repo");
}

#[test]
fn shell_pre_and_tool_post_keep_their_own_fields() {
  let shell = parse(with(tool("shell/pre"), &json!({ "command": "ls -la" }))).unwrap();
  let NormalizedEvent::ShellPre { command, .. } = &shell else {
    panic!("not shell/pre: {shell:?}");
  };
  assert_eq!(command.as_str(), "ls -la");
  let post = parse(with(tool("tool/post"), &json!({ "toolOutput": "Done" }))).unwrap();
  let NormalizedEvent::ToolPost { output, .. } = &post else {
    panic!("not tool/post: {post:?}");
  };
  assert_eq!(output, &Some(json!("Done")));
}

#[test]
fn a_missing_tool_input_defaults_to_an_empty_object() {
  let mut doc = tool("tool/pre");
  doc.as_object_mut().unwrap().remove("toolInput");
  let event = parse(doc).unwrap();
  assert!(event.tool().unwrap().input.is_empty());
  let written = serde_json::to_value(&event).unwrap();
  assert_eq!(written.get("toolInput"), Some(&json!({})));
}

#[test]
fn a_null_tool_output_reads_as_absent() {
  let event = parse(with(tool("tool/post"), &json!({ "toolOutput": null }))).unwrap();
  let NormalizedEvent::ToolPost { output, .. } = event else {
    panic!("not tool/post");
  };
  assert_eq!(output, None);
}

#[test]
fn a_missing_session_id_is_refused() {
  let mut doc = tool("tool/pre");
  doc.as_object_mut().unwrap().remove("sessionId");
  let err = parse(doc).unwrap_err();
  assert!(err.contains("missing field `sessionId`"), "{err}");
}

#[test]
fn an_unknown_field_is_refused() {
  let err = parse(with(base("session/start"), &json!({ "futureField": 1 }))).unwrap_err();
  assert!(err.contains("unknown field `futureField`"), "{err}");
}

#[test]
fn a_field_of_another_type_is_refused() {
  let err = parse(with(tool("tool/pre"), &json!({ "command": "ls" }))).unwrap_err();
  assert!(
    err.contains("`command` is not a field of tool/pre"),
    "{err}"
  );
  assert!(parse(with(tool("tool/pre"), &json!({ "toolOutput": 1 }))).is_err());
  assert!(parse(with(base("session/start"), &json!({ "prompt": "x" }))).is_err());
  assert!(
    parse(with(
      base("prompt"),
      &json!({ "prompt": "x", "toolName": "Bash" })
    ))
    .is_err()
  );
}

#[test]
fn a_missing_type_field_is_refused() {
  let err = parse(base("shell/pre")).unwrap_err();
  assert!(err.contains("shell/pre needs `toolCallId`"), "{err}");
  assert!(parse(tool("shell/pre")).is_err());
  assert!(parse(base("prompt")).is_err());
  assert!(parse(base("permission/evaluate")).is_err());
}

#[test]
fn an_empty_required_string_or_an_unknown_type_is_refused() {
  assert!(parse(with(tool("tool/pre"), &json!({ "toolName": "" }))).is_err());
  assert!(parse(with(base("session/start"), &json!({ "cwd": "" }))).is_err());
  assert!(parse(base("tool/during")).is_err());
}

#[test]
fn an_empty_prompt_is_allowed_like_zod_string() {
  let event = parse(with(base("prompt"), &json!({ "prompt": "" }))).unwrap();
  assert_eq!(event.kind(), EventKind::Prompt);
}

#[test]
fn a_built_shell_event_writes_the_typescript_wire() {
  let text = |value: &str| Text::new(value).unwrap();
  let event = NormalizedEvent::ShellPre {
    session: Session {
      session_id: text("s1"),
      cwd: text("/repo/src"),
      project_root: text("/repo"),
      worktree: text("/repo"),
    },
    tool: Tool {
      call_id: text("c1"),
      name: text("Bash"),
      input: json!({ "command": "ls" }).as_object().unwrap().clone(),
    },
    command: text("ls"),
  };
  assert_eq!(
    serde_json::to_value(&event).unwrap(),
    json!({
      "type": "shell/pre",
      "sessionId": "s1",
      "cwd": "/repo/src",
      "projectRoot": "/repo",
      "worktree": "/repo",
      "toolCallId": "c1",
      "toolName": "Bash",
      "toolInput": { "command": "ls" },
      "command": "ls",
    })
  );
}
