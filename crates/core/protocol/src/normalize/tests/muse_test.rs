//! Muse payloads (`docs/research/muse-tool-inventory.md`, #540) normalize as
//! Claude's do once their `snake_case` tool names take Claude's.

use serde_json::json;

use super::{MUSE_TOOLS, tool_name};
use crate::event::{EventKind, HostEvent};
use crate::host::Host;
use crate::normalize::Roots;
use crate::normalized::NormalizedEvent;
use crate::payload::parse;
use crate::text::Text;

fn muse(event: HostEvent, json: &str) -> NormalizedEvent {
  let roots = Roots {
    project_root: Text::new("/project").unwrap(),
    worktree: Text::new("/worktree").unwrap(),
  };
  parse(Host::Muse, json)
    .unwrap()
    .normalize(event, &roots)
    .unwrap()
}

#[test]
fn each_muse_tool_with_a_claude_counterpart_takes_its_name() {
  let pairs = [
    ("bash", "Bash"),
    ("shell", "Bash"),
    ("read_file", "Read"),
    ("write_file", "Write"),
    ("edit_file", "Edit"),
    ("search", "Grep"),
    ("web_fetch", "WebFetch"),
    ("web_search", "WebSearch"),
    ("read_skill", "Skill"),
    ("subagent_spawn", "Agent"),
    ("work_stop", "TaskStop"),
    ("send_session_message", "SendMessage"),
    ("write_todos", "TodoWrite"),
  ];
  for (muse, canonical) in pairs {
    assert_eq!(tool_name(muse), canonical, "{muse}");
  }
  assert_eq!(MUSE_TOOLS.len(), 20);
}

#[test]
fn any_other_name_passes_through_verbatim_and_case_sensitively() {
  for name in [
    "submit_reminder_decision",
    "submit_result",
    "bash_input",
    "read_memory",
    "mcp__plugin_toolu_probe_540_probe_mcp__probe_echo",
    "Bash",
    "BASH",
    "",
  ] {
    assert_eq!(tool_name(name), name);
  }
}

#[test]
fn a_muse_bash_call_with_a_command_is_shell_pre_for_bash() {
  let event = muse(
    HostEvent::ToolPre,
    r#"{"session_id":"s1","cwd":"/ws","hook_event_name":"PreToolUse","model":"m",
        "permission_mode":"bypassPermissions","transcript_path":null,"turn_id":"t",
        "tool_name":"bash","tool_use_id":"c1",
        "tool_input":{"command":"ls","description":"list","workdir":"/ws"}}"#,
  );
  let NormalizedEvent::ShellPre { tool, command, .. } = &event else {
    panic!("not shell/pre: {event:?}");
  };
  assert_eq!(command.as_str(), "ls");
  assert_eq!(tool.name.as_str(), "Bash");
  assert_eq!(tool.call_id.as_str(), "c1");
  assert_eq!(tool.input.get("workdir"), Some(&json!("/ws")));
}

#[test]
fn a_muse_bash_call_without_a_string_command_is_tool_pre_for_bash() {
  for doc in [
    r#"{"tool_name":"bash","tool_input":{"description":"none"}}"#,
    r#"{"tool_name":"bash","tool_input":{"command":7}}"#,
  ] {
    let event = muse(HostEvent::ToolPre, doc);
    assert_eq!(event.kind(), EventKind::ToolPre, "{doc}");
    assert_eq!(event.tool().unwrap().name.as_str(), "Bash");
  }
}

#[test]
fn a_muse_edit_keeps_its_own_input_keys() {
  let event = muse(
    HostEvent::ToolPre,
    r#"{"tool_name":"edit_file","tool_input":{"find":"two","path":"a.txt","replace":"three"}}"#,
  );
  let tool = event.tool().unwrap();
  assert_eq!(tool.name.as_str(), "Edit");
  assert_eq!(
    serde_json::Value::Object(tool.input.clone()),
    json!({ "find": "two", "path": "a.txt", "replace": "three" })
  );
}

#[test]
fn a_permission_request_names_the_canonical_tool_and_has_no_call_id() {
  let doc = r#"{"tool_name":"bash","tool_input":{"command":"touch a"},"turn_id":"t"}"#;
  let NormalizedEvent::PermissionEvaluate { permission, .. } =
    muse(HostEvent::PermissionEvaluate, doc)
  else {
    panic!("not permission/evaluate");
  };
  assert_eq!(permission.as_str(), "Bash");
  let pre = muse(HostEvent::ToolPre, doc);
  assert_eq!(pre.tool().unwrap().call_id.as_str(), "unknown");
}

#[test]
fn a_missing_muse_tool_name_is_unknown_as_on_claude() {
  let event = muse(HostEvent::ToolPost, r#"{"tool_response":"ok"}"#);
  assert_eq!(event.tool().unwrap().name.as_str(), "unknown");
}

#[test]
fn claude_and_codex_names_are_never_canonicalized() {
  let doc = r#"{"tool_name":"bash","tool_input":{"command":"ls"}}"#;
  for host in [Host::Claude, Host::Codex] {
    let roots = Roots {
      project_root: Text::new("/p").unwrap(),
      worktree: Text::new("/p").unwrap(),
    };
    let event = parse(host, doc)
      .unwrap()
      .normalize(HostEvent::ToolPre, &roots)
      .unwrap();
    assert_eq!(event.kind(), EventKind::ToolPre, "{host:?}");
    assert_eq!(event.tool().unwrap().name.as_str(), "bash");
  }
}
