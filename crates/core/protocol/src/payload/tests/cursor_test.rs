//! Cursor stdin as `tools/toolu-conformance/src/harness/fixtures.ts` renders it.

use serde_json::json;

use super::CursorPayload;

fn parse(json: &str) -> CursorPayload {
  serde_json::from_str(json).unwrap()
}

#[test]
fn before_shell_execution_reads_its_command() {
  let payload = parse(
    r#"{"conversation_id":"c1","hook_event_name":"beforeShellExecution",
        "workspace_roots":["/repo"],"command":"rm -rf build","cwd":"/repo","sandbox":false}"#,
  );
  assert_eq!(payload.conversation_id.as_str(), Some("c1"));
  assert_eq!(
    payload.hook_event_name.as_str(),
    Some("beforeShellExecution")
  );
  assert_eq!(payload.command.as_str(), Some("rm -rf build"));
  assert_eq!(payload.cwd.as_str(), Some("/repo"));
  assert_eq!(payload.workspace_roots, Some(json!(["/repo"])));
  assert_eq!(payload.sandbox, Some(json!(false)));
}

#[test]
fn before_mcp_execution_keeps_its_tool_input_as_a_json_string() {
  let payload = parse(
    r#"{"conversation_id":"c1","hook_event_name":"beforeMCPExecution","workspace_roots":["/r"],
        "tool_name":"search","tool_input":"{\"q\":\"x\"}","mcp_server_name":"canva"}"#,
  );
  assert_eq!(payload.tool_name.as_str(), Some("search"));
  assert_eq!(payload.tool_input, Some(json!("{\"q\":\"x\"}")));
  assert_eq!(payload.mcp_server_name.as_str(), Some("canva"));
}

#[test]
fn pre_tool_use_post_tool_use_and_session_fields_read() {
  let pre = parse(
    r#"{"conversation_id":"c1","hook_event_name":"preToolUse","tool_name":"Write",
        "tool_input":{"file_path":"/r/a"},"tool_use_id":"call-1","cwd":"/r"}"#,
  );
  assert_eq!(pre.tool_use_id.as_str(), Some("call-1"));
  assert_eq!(pre.tool_input, Some(json!({ "file_path": "/r/a" })));
  let post = parse(r#"{"hook_event_name":"postToolUse","tool_output":"{\"ok\":1}"}"#);
  assert_eq!(post.tool_output, Some(json!("{\"ok\":1}")));
  let start = parse(r#"{"hook_event_name":"sessionStart","session_id":"s9"}"#);
  assert_eq!(start.session_id.as_str(), Some("s9"));
  let prompt = parse(r#"{"hook_event_name":"beforeSubmitPrompt","prompt":"fix it"}"#);
  assert_eq!(prompt.prompt, Some(json!("fix it")));
}

#[test]
fn undocumented_cursor_fields_stay_in_rest() {
  let payload = parse(r#"{"generation_id":"g1","cursor_version":"1.7.0"}"#);
  assert_eq!(payload.rest.get("generation_id"), Some(&json!("g1")));
  assert_eq!(payload.rest.get("cursor_version"), Some(&json!("1.7.0")));
}
