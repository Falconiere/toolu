use serde_json::{Value, json};

use super::{Call, matcher_subject};
use crate::opencode::Hook;

fn call(doc: &Value) -> Call {
  Call::parse(&doc.to_string()).unwrap()
}

fn payload(hook: Hook, doc: &Value) -> Value {
  Value::Object(call(doc).payload(hook))
}

#[test]
fn a_before_call_becomes_a_claude_pre_tool_use_payload() {
  let doc = json!({"directory": "/repo", "event": {"tool": "edit", "sessionID": "ses_1",
    "agent": "build", "messageID": "msg_1", "id": "call_1",
    "input": {"path": "/repo/a.txt", "oldString": "hi", "newString": "hello"}}});
  assert_eq!(
    payload(Hook::ToolBefore, &doc),
    json!({"hook_event_name": "PreToolUse", "session_id": "ses_1", "cwd": "/repo",
      "tool_name": "Edit", "tool_use_id": "call_1",
      "tool_input": {"path": "/repo/a.txt", "oldString": "hi", "newString": "hello",
        "file_path": "/repo/a.txt", "old_string": "hi", "new_string": "hello"}})
  );
}

#[test]
fn a_shell_call_runs_in_its_workdir() {
  let doc = json!({"directory": "/repo", "event": {"tool": "shell", "sessionID": "s",
    "id": "c", "input": {"command": "git status", "workdir": "crates"}}});
  let out = payload(Hook::ToolBefore, &doc);
  assert_eq!(out["cwd"], "/repo/crates");
  assert_eq!(out["tool_name"], "Bash");
  let absolute = json!({"event": {"tool": "shell", "input": {"workdir": "/elsewhere"}}});
  assert_eq!(call(&absolute).cwd().as_deref(), Some("/elsewhere"));
  assert_eq!(call(&json!({"event": {}})).cwd(), None);
}

#[test]
fn a_completed_after_call_carries_its_output_metadata_text_and_exit_code() {
  let doc = json!({"directory": "/repo", "event": {"tool": "shell", "sessionID": "s", "id": "c",
    "input": {"command": "false"}, "status": "completed",
    "result": {"output": {"output": "boom"}, "metadata": {"exit": 1},
      "content": [{"type": "text", "text": "boom"}, {"type": "file"}]}}});
  let out = payload(Hook::ToolAfter, &doc);
  assert_eq!(out["hook_event_name"], "PostToolUse");
  assert_eq!(
    out["tool_response"],
    json!({"output": "boom", "metadata": {"exit": 1}, "content": "boom", "exit_code": 1})
  );
}

#[test]
fn a_scalar_or_missing_output_and_a_failed_call_still_respond() {
  let scalar = json!({"event": {"tool": "read", "status": "completed",
    "result": {"output": "text"}}});
  assert_eq!(
    payload(Hook::ToolAfter, &scalar)["tool_response"],
    json!({"output": "text"})
  );
  let missing = json!({"event": {"tool": "read", "status": "completed"}});
  assert_eq!(
    payload(Hook::ToolAfter, &missing)["tool_response"],
    json!({})
  );
  let failed = json!({"event": {"tool": "shell", "status": "error",
    "error": {"message": "denied"}}});
  assert_eq!(
    payload(Hook::ToolAfter, &failed)["tool_response"],
    json!({"error": "denied"})
  );
  let bare = json!({"event": {"status": "error", "error": "gone"}});
  assert_eq!(
    payload(Hook::ToolAfter, &bare)["tool_response"],
    json!({"error": "gone"})
  );
}

#[test]
fn session_events_carry_source_prompt_and_trigger() {
  let start = json!({"directory": "/repo", "event": {"sessionID": "s"}});
  assert_eq!(
    payload(Hook::SessionStart, &start),
    json!({"hook_event_name": "SessionStart", "session_id": "s", "cwd": "/repo",
      "source": "startup"})
  );
  let resume = json!({"event": {"sessionID": "s", "source": "resume"}});
  assert_eq!(payload(Hook::SessionStart, &resume)["source"], "resume");
  let prompt = json!({"event": {"sessionID": "s", "prompt": {"text": "fix it", "files": []}}});
  assert_eq!(payload(Hook::Prompt, &prompt)["prompt"], "fix it");
  assert_eq!(payload(Hook::Prompt, &json!({"event": {}}))["prompt"], "");
  let compact = payload(Hook::Compaction, &json!({"event": {"sessionID": "s"}}));
  assert_eq!(compact["trigger"], "auto");
  assert_eq!(compact["hook_event_name"], "PreCompact");
}

#[test]
fn matchers_test_the_tool_source_or_trigger() {
  let tool = call(&json!({"event": {"tool": "patch"}})).payload(Hook::ToolBefore);
  assert_eq!(
    matcher_subject(Hook::ToolBefore, &tool),
    Some("apply_patch")
  );
  let start = call(&json!({"event": {}})).payload(Hook::SessionStart);
  assert_eq!(matcher_subject(Hook::SessionStart, &start), Some("startup"));
  let compact = call(&json!({"event": {}})).payload(Hook::Compaction);
  assert_eq!(matcher_subject(Hook::Compaction, &compact), Some("auto"));
  let prompt = call(&json!({"event": {}})).payload(Hook::Prompt);
  assert_eq!(matcher_subject(Hook::Prompt, &prompt), None);
}

#[test]
fn a_call_that_is_not_an_object_with_an_event_object_is_an_error() {
  for text in ["", "[]", "{}", r#"{"event": 1}"#, r#"{"event": []}"#] {
    assert!(Call::parse(text).is_err(), "{text:?}");
  }
  let empty_dir = Call::parse(r#"{"directory": "", "event": {}}"#).unwrap();
  assert_eq!(empty_dir.directory, None);
}
