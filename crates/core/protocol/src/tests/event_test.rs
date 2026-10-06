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

use super::{EventKind, HostEvent};

#[test]
fn host_events_keep_the_typescript_order_and_slugs() {
  let slugs: Vec<&str> = HostEvent::ALL.iter().map(|event| event.slug()).collect();
  assert_eq!(
    slugs,
    [
      "session/start",
      "session/unload",
      "prompt",
      "pre_compact",
      "permission/evaluate",
      "tool/pre",
      "shell/pre",
      "tool/post",
    ]
  );
  for event in HostEvent::ALL {
    assert_eq!(HostEvent::from_slug(event.slug()), Some(event));
  }
}

#[test]
fn an_unknown_host_event_slug_is_none() {
  for slug in ["session/resume", "compaction", "PreToolUse", "tool/Pre", ""] {
    assert_eq!(HostEvent::from_slug(slug), None, "{slug:?}");
  }
}

#[test]
fn event_kinds_are_the_eleven_bridge_slugs_on_the_wire() {
  let slugs: Vec<&str> = EventKind::ALL.iter().map(|kind| kind.slug()).collect();
  assert_eq!(
    slugs,
    [
      "session/start",
      "session/resume",
      "session/clear",
      "session/unload",
      "prompt",
      "pre_compact",
      "compaction",
      "permission/evaluate",
      "tool/pre",
      "tool/post",
      "shell/pre",
    ]
  );
  for kind in EventKind::ALL {
    let json = serde_json::to_string(&kind).unwrap();
    assert_eq!(json, format!("\"{}\"", kind.slug()));
    assert_eq!(serde_json::from_str::<EventKind>(&json).unwrap(), kind);
  }
}

#[test]
fn an_unknown_event_kind_fails_to_parse() {
  let err = serde_json::from_str::<EventKind>(r#""tool/during""#).unwrap_err();
  assert!(
    err.to_string().contains("unknown event type `tool/during`"),
    "{err}"
  );
}
