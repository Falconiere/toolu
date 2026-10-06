use serde_json::json;

use super::HookPayload;

fn parse(json: &str) -> HookPayload {
  serde_json::from_str(json).unwrap()
}

#[test]
fn a_claude_pre_tool_use_payload_reads_its_known_fields() {
  let payload = parse(
    r#"{"session_id":"s1","transcript_path":"/t.jsonl","cwd":"/repo","permission_mode":"default",
        "hook_event_name":"PreToolUse","tool_name":"Bash","tool_input":{"command":"ls"},
        "tool_use_id":"toolu_1"}"#,
  );
  assert_eq!(payload.session_id.as_str(), Some("s1"));
  assert_eq!(payload.transcript_path.as_str(), Some("/t.jsonl"));
  assert_eq!(payload.cwd.as_str(), Some("/repo"));
  assert_eq!(payload.permission_mode.as_str(), Some("default"));
  assert_eq!(payload.hook_event_name.as_str(), Some("PreToolUse"));
  assert_eq!(payload.tool_name.as_str(), Some("Bash"));
  assert_eq!(payload.tool_use_id.as_str(), Some("toolu_1"));
  assert_eq!(payload.tool_input, Some(json!({ "command": "ls" })));
  assert!(payload.rest.is_empty());
}

#[test]
fn an_unknown_top_level_field_is_kept_in_rest() {
  let payload = parse(r#"{"hook_event_name":"PreToolUse","tool_name":"Bash","future_field":1}"#);
  assert_eq!(payload.rest.get("future_field"), Some(&json!(1)));
  assert_eq!(payload.tool_name.as_str(), Some("Bash"));
}

#[test]
fn codex_fields_and_post_tool_results_read() {
  let payload = parse(
    r#"{"session_id":"s","turn_id":"t1","model":"gpt-5","tool_name":"apply_patch",
        "tool_response":{"ok":true},"tool_output":"legacy"}"#,
  );
  assert_eq!(payload.turn_id.as_str(), Some("t1"));
  assert_eq!(payload.model.as_str(), Some("gpt-5"));
  assert_eq!(payload.tool_response, Some(json!({ "ok": true })));
  assert_eq!(payload.tool_output, Some(json!("legacy")));
}

#[test]
fn polymorphic_lifecycle_fields_keep_their_json_value() {
  assert_eq!(parse(r#"{"source":7}"#).source, Some(json!(7)));
  let fallthrough = parse(r#"{"source":false,"event":"resume"}"#);
  assert_eq!(fallthrough.source, Some(json!(false)));
  assert_eq!(fallthrough.event, Some(json!("resume")));
  assert_eq!(
    parse(r#"{"session_event":"resume"}"#).session_event,
    Some(json!("resume"))
  );
  assert_eq!(
    parse(r#"{"prompt":{"text":"fix it"}}"#).prompt,
    Some(json!({ "text": "fix it" }))
  );
  assert_eq!(parse(r#"{"prompt":null}"#).prompt, None);
}

#[test]
fn a_wrong_typed_scalar_reads_as_absent() {
  let payload = parse(r#"{"session_id":7,"cwd":null,"tool_name":["Bash"]}"#);
  assert_eq!(payload.session_id.as_str(), None);
  assert_eq!(payload.cwd.as_str(), None);
  assert_eq!(payload.tool_name.as_str(), None);
}

#[test]
fn session_end_and_pre_compact_fields_read() {
  let end = parse(r#"{"hook_event_name":"SessionEnd","reason":"clear"}"#);
  assert_eq!(end.reason.as_str(), Some("clear"));
  let compact = parse(r#"{"trigger":"manual","custom_instructions":"keep tests"}"#);
  assert_eq!(compact.trigger.as_str(), Some("manual"));
  assert_eq!(compact.custom_instructions.as_str(), Some("keep tests"));
}
