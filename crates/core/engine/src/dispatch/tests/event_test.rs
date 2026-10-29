use serde_json::json;
use toolu_protocol::event::HostEvent;
use toolu_protocol::normalized::NormalizedEvent;
use toolu_runtime::registry::rule::EditOperation as Runtime;
use toolu_state::edit_records::{EditOperation, EditRecord};

use super::{EditFields, View, host_event, runtime_operation, text};

#[test]
fn absent_or_empty_strings_take_the_fallback() {
  let value = json!({"a": "x", "e": "", "n": 1});
  assert_eq!(text(value.get("a"), "f"), "x");
  for key in ["e", "n", "missing"] {
    assert_eq!(text(value.get(key), "f"), "f", "{key}");
  }
}

#[test]
fn each_tool_event_is_encoded_for_its_host_event() {
  let wire = |kind: &str, extra: serde_json::Value| -> NormalizedEvent {
    let mut base = json!({"type": kind, "sessionId": "s", "cwd": "/p", "projectRoot": "/p", "worktree": "/p",
      "toolCallId": "c", "toolName": "Bash", "toolInput": {}});
    if let (Some(base), Some(extra)) = (base.as_object_mut(), extra.as_object()) {
      base.extend(extra.clone());
    }
    serde_json::from_value(base).unwrap()
  };
  assert_eq!(host_event(&wire("tool/pre", json!({}))), HostEvent::ToolPre);
  assert_eq!(
    host_event(&wire("shell/pre", json!({"command": "ls"}))),
    HostEvent::ShellPre
  );
  assert_eq!(
    host_event(&wire("tool/post", json!({}))),
    HostEvent::ToolPost
  );
}

#[test]
fn a_record_gives_its_edit_fields_with_empty_defaults() {
  let record = EditRecord {
    path: "b".to_owned(),
    operation: EditOperation::Move,
    moved_to: None,
    from: Some("a".to_owned()),
  };
  let fields = EditFields::of(&record);
  assert_eq!((fields.from.as_str(), fields.moved_to.as_str()), ("a", ""));
  let pairs = [
    (EditOperation::Add, Runtime::Add),
    (EditOperation::Update, Runtime::Update),
    (EditOperation::Delete, Runtime::Delete),
    (EditOperation::Write, Runtime::Write),
    (EditOperation::Move, Runtime::Move),
  ];
  for (state, runtime) in pairs {
    assert_eq!(runtime_operation(state), runtime);
  }
}

#[test]
fn a_shared_edit_view_updates_the_record_without_losing_the_patch() {
  let event = serde_json::from_value(json!({
    "type": "tool/post", "sessionId": "s", "cwd": "/p", "projectRoot": "/p",
    "worktree": "/p", "toolCallId": "c", "toolName": "Edit",
    "toolInput": {"command": "large patch", "path": "first.rs"}
  }))
  .unwrap();
  let mut view = View {
    event,
    raw: json!({"tool_input": {"command": "large patch", "path": "first.rs"}})
      .as_object()
      .unwrap()
      .clone(),
  };
  let record = EditRecord {
    path: "second.rs".to_owned(),
    operation: EditOperation::Move,
    moved_to: Some("third.rs".to_owned()),
    from: Some("first.rs".to_owned()),
  };
  view.set_record(&record);
  let input = &view.event.tool().unwrap().input;
  assert_eq!(input.get("command"), Some(&json!("large patch")));
  assert_eq!(input.get("path"), Some(&json!("second.rs")));
  assert_eq!(input.get("toolu_edit_from"), Some(&json!("first.rs")));
  assert_eq!(input.get("toolu_edit_moved_to"), Some(&json!("third.rs")));
  assert_eq!(view.raw["tool_input"]["path"], "second.rs");
}
