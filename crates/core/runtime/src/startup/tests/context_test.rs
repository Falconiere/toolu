use super::{MAX_CONTEXT_CHARS, render_hook_output, session_context};

#[test]
fn there_is_no_context_for_empty_text() {
  assert_eq!(session_context("SessionStart", ""), None);
}

#[test]
fn context_renders_with_jq_bytes_in_typescript_key_order() {
  let value = session_context("SessionStart", "hi\u{7f}").unwrap();
  let pretty = "{\n  \"hookSpecificOutput\": {\n    \"hookEventName\": \"SessionStart\",\n    \"additionalContext\": \"hi\\u007f\"\n  }\n}\n";
  assert_eq!(render_hook_output(&value, true), pretty);
  let compact = "{\"hookSpecificOutput\":{\"hookEventName\":\"SessionStart\",\"additionalContext\":\"hi\\u007f\"}}\n";
  assert_eq!(render_hook_output(&value, false), compact);
}

fn context_text(text: &str) -> String {
  let value = session_context("SessionStart", text).unwrap();
  let inner = value.get("hookSpecificOutput").unwrap();
  let Some(crate::json::ordered::Ordered::String(text)) = inner.get("additionalContext") else {
    panic!("no context string");
  };
  text.clone()
}

#[test]
fn text_is_cut_to_the_limit_in_utf16_units_without_splitting_a_pair() {
  let fits = "a".repeat(MAX_CONTEXT_CHARS);
  assert_eq!(context_text(&fits), fits);
  let straddles = format!("{}\u{1f600}", "a".repeat(MAX_CONTEXT_CHARS - 1));
  assert_eq!(context_text(&straddles), "a".repeat(MAX_CONTEXT_CHARS - 1));
  let pairs = "\u{1f600}".repeat(MAX_CONTEXT_CHARS);
  assert_eq!(
    context_text(&pairs).encode_utf16().count(),
    MAX_CONTEXT_CHARS
  );
}
