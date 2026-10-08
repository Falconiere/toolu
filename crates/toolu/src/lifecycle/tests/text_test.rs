use serde_json::json;

use super::{ascii_lower, jq_alt, member, parse_stdin, session_event};

#[test]
fn empty_invalid_and_missing_source_are_startup() {
  assert_eq!(session_event(""), "startup");
  assert_eq!(session_event("{not json"), "startup");
  assert_eq!(session_event("{}"), "startup");
  assert_eq!(
    session_event(r#"{"hook_event_name":"SessionStart","source":"startup"}"#),
    "startup"
  );
}

#[test]
fn a_false_source_falls_through_like_jq_or() {
  assert_eq!(
    session_event(r#"{"source":false,"event":"resume"}"#),
    "resume"
  );
  assert_eq!(session_event(r#"{"session_event":"resume"}"#), "resume");
  assert_eq!(session_event(r#"{"source":"null"}"#), "startup");
  assert_eq!(session_event(r#"{"source":7}"#), "7");
  assert_eq!(session_event(r#"{"source":"reboot"}"#), "reboot");
}

#[test]
fn ascii_lower_leaves_non_ascii_letters_alone() {
  assert_eq!(ascii_lower("Fix É"), "fix É");
}

#[test]
fn jq_alt_treats_null_false_and_absent_as_the_fallback() {
  let doc = parse_stdin(r#"{"source":false,"name":"toolu"}"#);
  assert_eq!(jq_alt(member(doc.as_ref(), "source"), "startup"), "startup");
  assert_eq!(
    jq_alt(member(doc.as_ref(), "missing"), "startup"),
    "startup"
  );
  assert_eq!(jq_alt(member(doc.as_ref(), "name"), "startup"), "toolu");
  assert_eq!(jq_alt(Some(&json!(null)), "startup"), "startup");
}
