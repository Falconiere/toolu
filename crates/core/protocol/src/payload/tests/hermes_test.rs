//! The shell-hook envelope documented at
//! <https://hermes-agent.nousresearch.com/docs/user-guide/features/hooks>.

use serde_json::json;

use super::HermesPayload;

#[test]
fn the_documented_envelope_reads_and_extra_stays_opaque() {
  let payload: HermesPayload = serde_json::from_str(
    r#"{"hook_event_name":"pre_tool_call","tool_name":"terminal",
        "tool_input":{"command":"ls"},"session_id":"s1","cwd":"/repo",
        "extra":{"tool_call_id":"t1","task_id":"k"}}"#,
  )
  .unwrap();
  assert_eq!(payload.hook_event_name.as_str(), Some("pre_tool_call"));
  assert_eq!(payload.tool_name.as_str(), Some("terminal"));
  assert_eq!(payload.session_id.as_str(), Some("s1"));
  assert_eq!(payload.cwd.as_str(), Some("/repo"));
  assert_eq!(payload.tool_input, Some(json!({ "command": "ls" })));
  assert_eq!(
    payload.extra,
    Some(json!({ "tool_call_id": "t1", "task_id": "k" }))
  );
}

#[test]
fn fields_outside_the_envelope_stay_in_rest() {
  let payload: HermesPayload =
    serde_json::from_str(r#"{"hook_event_name":"pre_llm_call","profile":"default"}"#).unwrap();
  assert_eq!(payload.rest.get("profile"), Some(&json!("default")));
}
