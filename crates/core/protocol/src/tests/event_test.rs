use super::{ENFORCING_EVENTS, is_enforcing};

#[test]
fn pre_tool_use_and_permission_request_enforce() {
  assert_eq!(ENFORCING_EVENTS, ["PreToolUse", "PermissionRequest"]);
  assert!(is_enforcing("PreToolUse"));
  assert!(is_enforcing("PermissionRequest"));
}

#[test]
fn context_events_and_near_misses_do_not_enforce() {
  for event in [
    "SessionStart",
    "PostToolUse",
    "UserPromptSubmit",
    "pretooluse",
    "",
  ] {
    assert!(!is_enforcing(event), "{event}");
  }
}
