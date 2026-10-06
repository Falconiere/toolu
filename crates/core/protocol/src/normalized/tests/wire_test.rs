use super::{allowed, need};
use crate::event::EventKind;

#[test]
fn session_types_carry_no_optional_field() {
  for kind in [
    EventKind::SessionStart,
    EventKind::SessionResume,
    EventKind::SessionClear,
    EventKind::SessionUnload,
    EventKind::PreCompact,
    EventKind::Compaction,
  ] {
    assert!(allowed(kind).is_empty(), "{kind:?}");
  }
}

#[test]
fn each_tool_type_adds_only_its_own_extra() {
  assert_eq!(
    allowed(EventKind::ToolPre),
    ["toolCallId", "toolName", "toolInput"]
  );
  assert!(allowed(EventKind::ToolPost).contains(&"toolOutput"));
  assert!(!allowed(EventKind::ToolPost).contains(&"command"));
  assert!(allowed(EventKind::ShellPre).contains(&"command"));
  assert!(!allowed(EventKind::ShellPre).contains(&"toolOutput"));
  assert_eq!(allowed(EventKind::Prompt), ["prompt"]);
  assert_eq!(allowed(EventKind::PermissionEvaluate), ["permission"]);
}

#[test]
fn a_missing_required_field_names_the_type_and_the_field() {
  assert_eq!(need(Some(1), "prompt", EventKind::Prompt), Ok(1));
  assert_eq!(
    need::<u8>(None, "command", EventKind::ShellPre),
    Err("shell/pre needs `command`".to_owned())
  );
}
