//! The cases of `packages/toolu-core/src/host/__tests__/host-events.test.ts`.

use super::{canonical_event, hosts_for_native, native_event};
use crate::event::HostEvent;
use crate::event::HostEvent::{
  PermissionEvaluate, PreCompact, Prompt, SessionStart, SessionUnload, ShellPre, ToolPost, ToolPre,
};
use crate::host::Host;

#[test]
fn claude_and_codex_share_pascal_case_names() {
  for host in [Host::Claude, Host::Codex] {
    assert_eq!(native_event(host, ToolPre), Some("PreToolUse"));
    assert_eq!(native_event(host, ShellPre), Some("PreToolUse"));
    assert_eq!(native_event(host, ToolPost), Some("PostToolUse"));
    assert_eq!(native_event(host, SessionStart), Some("SessionStart"));
    assert_eq!(native_event(host, SessionUnload), Some("SessionEnd"));
    assert_eq!(native_event(host, Prompt), Some("UserPromptSubmit"));
    assert_eq!(native_event(host, PreCompact), Some("PreCompact"));
    assert_eq!(
      native_event(host, PermissionEvaluate),
      Some("PermissionRequest")
    );
  }
}

#[test]
fn cursor_uses_camel_case_and_splits_shell_out_of_pre_tool_use() {
  assert_eq!(native_event(Host::Cursor, ToolPre), Some("preToolUse"));
  assert_eq!(
    native_event(Host::Cursor, ShellPre),
    Some("beforeShellExecution")
  );
  assert_eq!(native_event(Host::Cursor, ToolPost), Some("postToolUse"));
  assert_eq!(
    native_event(Host::Cursor, Prompt),
    Some("beforeSubmitPrompt")
  );
  assert_eq!(
    native_event(Host::Cursor, SessionStart),
    Some("sessionStart")
  );
  assert_eq!(
    native_event(Host::Cursor, SessionUnload),
    Some("sessionEnd")
  );
  assert_eq!(native_event(Host::Cursor, PreCompact), Some("preCompact"));
  assert_eq!(native_event(Host::Cursor, PermissionEvaluate), None);
}

#[test]
fn hermes_uses_snake_case_and_has_no_compaction_or_permission_event() {
  assert_eq!(native_event(Host::Hermes, ToolPre), Some("pre_tool_call"));
  assert_eq!(native_event(Host::Hermes, ShellPre), Some("pre_tool_call"));
  assert_eq!(native_event(Host::Hermes, ToolPost), Some("post_tool_call"));
  assert_eq!(native_event(Host::Hermes, Prompt), Some("pre_llm_call"));
  assert_eq!(
    native_event(Host::Hermes, SessionStart),
    Some("on_session_start")
  );
  assert_eq!(
    native_event(Host::Hermes, SessionUnload),
    Some("on_session_end")
  );
  assert_eq!(native_event(Host::Hermes, PreCompact), None);
  assert_eq!(native_event(Host::Hermes, PermissionEvaluate), None);
}

#[test]
fn opencode_uses_dotted_plugin_hook_names() {
  assert_eq!(
    native_event(Host::Opencode, ToolPre),
    Some("tool.execute.before")
  );
  assert_eq!(
    native_event(Host::Opencode, ShellPre),
    Some("tool.execute.before")
  );
  assert_eq!(
    native_event(Host::Opencode, ToolPost),
    Some("tool.execute.after")
  );
  assert_eq!(native_event(Host::Opencode, PermissionEvaluate), None);
  assert_eq!(
    native_event(Host::Opencode, SessionStart),
    Some("session.created")
  );
  assert_eq!(
    native_event(Host::Opencode, SessionUnload),
    Some("session.deleted")
  );
  assert_eq!(native_event(Host::Opencode, Prompt), Some("chat.message"));
  assert_eq!(
    native_event(Host::Opencode, PreCompact),
    Some("experimental.session.compacting")
  );
}

#[test]
fn every_mapped_row_round_trips_to_an_event_with_the_same_native_name() {
  for host in Host::ALL {
    for event in HostEvent::ALL {
      let Some(native) = native_event(host, event) else {
        continue;
      };
      let back = canonical_event(host, native).unwrap();
      assert_eq!(native_event(host, back), Some(native), "{host:?} {event:?}");
    }
  }
}

#[test]
fn a_shared_native_name_reverses_to_the_first_row() {
  assert_eq!(canonical_event(Host::Claude, "PreToolUse"), Some(ToolPre));
  assert_eq!(
    canonical_event(Host::Hermes, "pre_tool_call"),
    Some(ToolPre)
  );
  assert_eq!(
    canonical_event(Host::Opencode, "tool.execute.before"),
    Some(ToolPre)
  );
}

#[test]
fn cursor_aliases_fold_into_tool_pre_and_tool_post() {
  assert_eq!(
    canonical_event(Host::Cursor, "beforeMCPExecution"),
    Some(ToolPre)
  );
  assert_eq!(
    canonical_event(Host::Cursor, "afterFileEdit"),
    Some(ToolPost)
  );
  assert_eq!(
    canonical_event(Host::Cursor, "afterShellExecution"),
    Some(ToolPost)
  );
  assert_eq!(
    canonical_event(Host::Cursor, "afterMCPExecution"),
    Some(ToolPost)
  );
  assert_eq!(canonical_event(Host::Claude, "afterFileEdit"), None);
}

#[test]
fn names_from_another_host_or_unknown_names_are_none() {
  assert_eq!(canonical_event(Host::Claude, "preToolUse"), None);
  assert_eq!(canonical_event(Host::Cursor, "PreToolUse"), None);
  assert_eq!(canonical_event(Host::Codex, "Notification"), None);
}

#[test]
fn pascal_case_names_belong_to_both_claude_and_codex() {
  assert_eq!(hosts_for_native("PreToolUse"), [Host::Claude, Host::Codex]);
}

#[test]
fn cursor_hermes_and_opencode_names_belong_to_one_host() {
  assert_eq!(hosts_for_native("beforeShellExecution"), [Host::Cursor]);
  assert_eq!(hosts_for_native("afterFileEdit"), [Host::Cursor]);
  assert_eq!(hosts_for_native("pre_tool_call"), [Host::Hermes]);
  assert_eq!(hosts_for_native("tool.execute.before"), [Host::Opencode]);
}

#[test]
fn an_unknown_name_belongs_to_no_host() {
  assert_eq!(hosts_for_native("pre_compact"), []);
  assert_eq!(hosts_for_native(""), []);
}
