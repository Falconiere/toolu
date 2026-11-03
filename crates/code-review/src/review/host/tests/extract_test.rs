use super::extract;
use toolu_protocol::host::Host;

const CLAUDE_OK: &str = r#"{"type":"result","subtype":"success","is_error":false,"result":"{\"findings\": []}","total_cost_usd":0.5,"usage":{"input_tokens":10,"output_tokens":7}}"#;
const CLAUDE_ERR: &str = r#"{"type":"result","subtype":"error_max_budget_usd","is_error":true,"result":"","errors":["Reached maximum budget ($0.25)"]}"#;
const CURSOR_OK: &str = r#"{"type":"result","subtype":"success","is_error":false,"result":"PING-OK","usage":{"inputTokens":16918,"outputTokens":140}}"#;
const OPENCODE_ERR: &str =
  r#"{"type":"error","error":{"type":"provider.auth","message":"User not found."}}"#;

#[test]
fn claude_success_reports_text_tokens_and_cost() {
  let found = extract(Host::Claude, CLAUDE_OK).unwrap();
  assert_eq!(found.text, "{\"findings\": []}");
  assert_eq!((found.tokens_in, found.tokens_out), (10, 7));
  assert_eq!(found.cost_usd, Some(0.5));
}

#[test]
fn claude_error_envelope_is_a_failure() {
  assert!(extract(Host::Claude, CLAUDE_ERR).is_err());
}

#[test]
fn cursor_reports_envelope_usage_without_cost() {
  let found = extract(Host::Cursor, CURSOR_OK).unwrap();
  assert_eq!(found.text, "PING-OK");
  assert_eq!((found.tokens_in, found.tokens_out), (16918, 140));
  assert_eq!(found.cost_usd, None);
}

#[test]
fn opencode_error_envelope_is_a_failure_despite_exit_zero() {
  assert!(extract(Host::Opencode, OPENCODE_ERR).is_err());
}

#[test]
fn codex_and_muse_pass_raw_json_through() {
  for host in [Host::Codex, Host::Muse] {
    let found = extract(host, "{\"ping\": \"PING-OK\"}").unwrap();
    assert_eq!(found.text, "{\"ping\": \"PING-OK\"}");
    assert_eq!((found.tokens_in, found.tokens_out), (0, 0));
  }
  assert!(extract(Host::Codex, "").is_err());
  assert!(extract(Host::Codex, "not json at all {{{").is_err());
}

#[test]
fn claude_prefers_structured_output_to_result_text() {
  let output = r#"{"type":"result","is_error":false,"result":"unstructured text","structured_output":{"findings":[]},"usage":{"input_tokens":10,"output_tokens":7},"total_cost_usd":0.5}"#;
  let found = extract(Host::Claude, output).unwrap();
  assert_eq!(found.text, r#"{"findings":[]}"#);
  assert_eq!((found.tokens_in, found.tokens_out), (10, 7));
  assert_eq!(found.cost_usd, Some(0.5));
}

#[test]
fn claude_accepts_structured_output_without_result_text() {
  let output = r#"{"type":"result","is_error":false,"structured_output":{"findings":[]}}"#;
  let found = extract(Host::Claude, output).unwrap();
  assert_eq!(found.text, r#"{"findings":[]}"#);
}

#[test]
fn claude_input_usage_includes_cached_tokens() {
  let body = r#"{"is_error":false,"result":"{}","usage":{"input_tokens":2,"cache_creation_input_tokens":45634,"cache_read_input_tokens":9,"output_tokens":77}}"#;
  let found = extract(Host::Claude, body).unwrap();
  assert_eq!((found.tokens_in, found.tokens_out), (45645, 77));
}
