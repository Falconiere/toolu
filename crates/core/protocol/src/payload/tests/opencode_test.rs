//! The `(input, output)` pair of `@opencode-ai/plugin@1.18.34` hooks.

use serde_json::json;

use super::OpencodePayload;

#[test]
fn tool_execute_before_reads_its_input_and_output() {
  let payload: OpencodePayload = serde_json::from_str(
    r#"{"input":{"tool":"bash","sessionID":"ses_1","callID":"call_1"},
        "output":{"args":{"command":"ls"}}}"#,
  )
  .unwrap();
  assert_eq!(payload.input.tool.as_str(), Some("bash"));
  assert_eq!(payload.input.session_id.as_str(), Some("ses_1"));
  assert_eq!(payload.input.call_id.as_str(), Some("call_1"));
  assert_eq!(payload.output, Some(json!({ "args": { "command": "ls" } })));
}

#[test]
fn tool_execute_after_carries_its_args_on_the_input() {
  let payload: OpencodePayload = serde_json::from_str(
    r#"{"input":{"tool":"read","sessionID":"s","callID":"c","args":{"filePath":"/a"}},
        "output":{"title":"a","output":"hello","metadata":{}}}"#,
  )
  .unwrap();
  assert_eq!(payload.input.args, Some(json!({ "filePath": "/a" })));
}

#[test]
fn unknown_input_and_top_level_keys_stay_in_rest() {
  let payload: OpencodePayload =
    serde_json::from_str(r#"{"input":{"agent":"build"},"hook":"chat.message"}"#).unwrap();
  assert_eq!(payload.input.rest.get("agent"), Some(&json!("build")));
  assert_eq!(payload.rest.get("hook"), Some(&json!("chat.message")));
}
