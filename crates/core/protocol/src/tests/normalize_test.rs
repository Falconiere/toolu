//! `toolEvent` (`dispatch-context.ts`) and `sessionEvent` (`session-start.ts`)
//! on real payload shapes.

use serde_json::json;

use super::{Roots, text_or};
use crate::event::{EventKind, HostEvent};
use crate::host::Host;
use crate::normalized::NormalizedEvent;
use crate::payload::parse;
use crate::text::Text;

fn roots() -> Roots {
  Roots {
    project_root: Text::new("/project").unwrap(),
    worktree: Text::new("/worktree").unwrap(),
  }
}

fn normalize(host: Host, event: HostEvent, json: &str) -> NormalizedEvent {
  parse(host, json)
    .unwrap()
    .normalize(event, &roots())
    .unwrap()
}

fn claude(event: HostEvent, json: &str) -> NormalizedEvent {
  normalize(Host::Claude, event, json)
}

#[test]
fn a_bash_call_with_a_command_is_shell_pre() {
  let event = claude(
    HostEvent::ToolPre,
    r#"{"session_id":"s1","cwd":"/repo/src","tool_name":"Bash","tool_use_id":"t1",
        "tool_input":{"command":"git push","description":"push"}}"#,
  );
  let NormalizedEvent::ShellPre {
    session,
    tool,
    command,
  } = &event
  else {
    panic!("not shell/pre: {event:?}");
  };
  assert_eq!(command.as_str(), "git push");
  assert_eq!(session.session_id.as_str(), "s1");
  assert_eq!(session.cwd.as_str(), "/repo/src");
  assert_eq!(session.project_root.as_str(), "/project");
  assert_eq!(session.worktree.as_str(), "/worktree");
  assert_eq!(tool.call_id.as_str(), "t1");
  assert_eq!(tool.name.as_str(), "Bash");
  assert_eq!(tool.input.get("description"), Some(&json!("push")));
}

#[test]
fn shell_is_a_shell_tool_too_and_the_payload_decides_not_the_event() {
  let doc = r#"{"tool_name":"Shell","tool_input":{"command":"ls"}}"#;
  assert_eq!(claude(HostEvent::ToolPre, doc).kind(), EventKind::ShellPre);
  assert_eq!(claude(HostEvent::ShellPre, doc).kind(), EventKind::ShellPre);
  let edit = r#"{"tool_name":"Edit","tool_input":{"file_path":"a"}}"#;
  assert_eq!(claude(HostEvent::ShellPre, edit).kind(), EventKind::ToolPre);
}

#[test]
fn a_bash_call_without_a_usable_command_is_tool_pre() {
  for doc in [
    r#"{"tool_name":"Bash","tool_input":{"command":""}}"#,
    r#"{"tool_name":"Bash","tool_input":{"command":7}}"#,
    r#"{"tool_name":"Bash","tool_input":"ls"}"#,
    r#"{"tool_name":"Bash"}"#,
  ] {
    assert_eq!(
      claude(HostEvent::ToolPre, doc).kind(),
      EventKind::ToolPre,
      "{doc}"
    );
  }
}

#[test]
fn missing_or_wrong_typed_ids_fall_back_like_typescript() {
  let event = claude(
    HostEvent::ToolPre,
    r#"{"session_id":7,"tool_use_id":"","tool_input":[1]}"#,
  );
  assert_eq!(event.session().session_id.as_str(), "unknown");
  assert_eq!(event.session().cwd.as_str(), "/project");
  let tool = event.tool().unwrap();
  assert_eq!(tool.call_id.as_str(), "unknown");
  assert_eq!(tool.name.as_str(), "unknown");
  assert!(tool.input.is_empty());
}

#[test]
fn post_tool_carries_tool_response_else_tool_output() {
  let output = |doc: &str| {
    let NormalizedEvent::ToolPost { output, .. } = claude(HostEvent::ToolPost, doc) else {
      panic!("not tool/post: {doc}");
    };
    output
  };
  assert_eq!(
    output(r#"{"tool_name":"Write","tool_response":{"ok":true},"tool_output":"x"}"#),
    Some(json!({ "ok": true }))
  );
  assert_eq!(
    output(r#"{"tool_name":"Write","tool_response":null,"tool_output":"x"}"#),
    Some(json!("x"))
  );
  assert_eq!(output(r#"{"tool_name":"Write"}"#), None);
}

#[test]
fn session_start_follows_source_then_session_event_then_event() {
  let kind = |doc: &str| claude(HostEvent::SessionStart, doc).kind();
  assert_eq!(kind(r#"{"source":"startup"}"#), EventKind::SessionStart);
  assert_eq!(kind(r#"{"source":"resume"}"#), EventKind::SessionResume);
  assert_eq!(kind(r#"{"source":"clear"}"#), EventKind::SessionClear);
  assert_eq!(kind(r#"{"source":"compact"}"#), EventKind::Compaction);
  assert_eq!(kind(r#"{"source":"reboot"}"#), EventKind::SessionStart);
  assert_eq!(kind(r#"{"source":7}"#), EventKind::SessionStart);
  assert_eq!(
    kind(r#"{"session_event":"resume"}"#),
    EventKind::SessionResume
  );
  assert_eq!(
    kind(r#"{"source":false,"event":"resume"}"#),
    EventKind::SessionResume
  );
  assert_eq!(
    kind(r#"{"source":null,"session_event":"clear"}"#),
    EventKind::SessionClear
  );
  assert_eq!(kind("{}"), EventKind::SessionStart);
}

#[test]
fn a_prompt_is_its_string_or_the_compact_json_of_anything_else() {
  let prompt = |doc: &str| {
    let NormalizedEvent::Prompt { prompt, .. } = claude(HostEvent::Prompt, doc) else {
      panic!("not prompt: {doc}");
    };
    prompt
  };
  assert_eq!(prompt(r#"{"prompt":"fix the bug"}"#), "fix the bug");
  assert_eq!(
    prompt(r#"{"prompt":{"text":"fix it"}}"#),
    r#"{"text":"fix it"}"#
  );
  assert_eq!(prompt(r#"{"prompt":42}"#), "42");
  assert_eq!(prompt(r#"{"prompt":null}"#), "");
  assert_eq!(prompt(r#"{"prompt":false}"#), "");
  assert_eq!(prompt("{}"), "");
}

#[test]
fn the_remaining_host_events_map_to_their_kinds() {
  assert_eq!(
    claude(HostEvent::SessionUnload, r#"{"reason":"exit"}"#).kind(),
    EventKind::SessionUnload
  );
  assert_eq!(
    claude(HostEvent::PreCompact, r#"{"trigger":"auto"}"#).kind(),
    EventKind::PreCompact
  );
  let permission = |doc: &str| {
    let NormalizedEvent::PermissionEvaluate { permission, .. } =
      claude(HostEvent::PermissionEvaluate, doc)
    else {
      panic!("not permission/evaluate: {doc}");
    };
    permission.as_str().to_owned()
  };
  assert_eq!(permission(r#"{"tool_name":"Edit"}"#), "Edit");
  assert_eq!(permission("{}"), "unknown");
}

#[test]
fn codex_reads_like_claude() {
  let event = normalize(
    Host::Codex,
    HostEvent::ToolPre,
    r#"{"session_id":"s","turn_id":"t","tool_name":"apply_patch","tool_use_id":"c",
        "tool_input":{"command":"*** Begin Patch"}}"#,
  );
  assert_eq!(event.kind(), EventKind::ToolPre);
  assert_eq!(event.tool().unwrap().name.as_str(), "apply_patch");
}

#[test]
fn cursor_before_shell_execution_is_the_shell_tool() {
  let event = normalize(
    Host::Cursor,
    HostEvent::ShellPre,
    r#"{"conversation_id":"conv","hook_event_name":"beforeShellExecution",
        "command":"rm -rf build","cwd":"/repo","sandbox":false}"#,
  );
  let NormalizedEvent::ShellPre {
    session,
    tool,
    command,
  } = &event
  else {
    panic!("not shell/pre: {event:?}");
  };
  assert_eq!(session.session_id.as_str(), "conv");
  assert_eq!(tool.name.as_str(), "Shell");
  assert_eq!(tool.input.get("command"), Some(&json!("rm -rf build")));
  assert_eq!(command.as_str(), "rm -rf build");
}

#[test]
fn cursor_reads_a_json_string_tool_input_and_prefers_session_id() {
  let mcp = normalize(
    Host::Cursor,
    HostEvent::ToolPre,
    r#"{"conversation_id":"conv","tool_name":"search","tool_input":"{\"q\":\"x\"}"}"#,
  );
  assert_eq!(mcp.tool().unwrap().input.get("q"), Some(&json!("x")));
  let text = normalize(
    Host::Cursor,
    HostEvent::ToolPre,
    r#"{"tool_name":"search","tool_input":"not json"}"#,
  );
  assert!(text.tool().unwrap().input.is_empty());
  let start = normalize(
    Host::Cursor,
    HostEvent::SessionStart,
    r#"{"session_id":"s9","conversation_id":"conv"}"#,
  );
  assert_eq!(start.session().session_id.as_str(), "s9");
}

#[test]
fn hermes_reads_only_its_top_level_envelope() {
  let event = normalize(
    Host::Hermes,
    HostEvent::ToolPre,
    r#"{"hook_event_name":"pre_tool_call","tool_name":"terminal","session_id":"s1",
        "cwd":"/repo","tool_input":{"command":"ls"},"extra":{"tool_call_id":"t1"}}"#,
  );
  assert_eq!(event.kind(), EventKind::ToolPre);
  assert_eq!(event.tool().unwrap().call_id.as_str(), "unknown");
  assert_eq!(event.session().cwd.as_str(), "/repo");
  let prompt = normalize(
    Host::Hermes,
    HostEvent::Prompt,
    r#"{"extra":{"user_message":"hi"}}"#,
  );
  assert!(matches!(prompt, NormalizedEvent::Prompt { ref prompt, .. } if prompt.is_empty()));
}

#[test]
fn opencode_payloads_do_not_normalize_until_the_tool_map_is_ported() {
  let payload = parse(
    Host::Opencode,
    r#"{"input":{"tool":"bash","sessionID":"s","callID":"c"},"output":{"args":{}}}"#,
  )
  .unwrap();
  assert_eq!(payload.normalize(HostEvent::ToolPre, &roots()), None);
}

#[test]
fn text_or_takes_a_non_empty_string_and_otherwise_its_fallback() {
  let fallback = Text::new("unknown").unwrap();
  assert_eq!(text_or(Some("s1"), &fallback).as_str(), "s1");
  assert_eq!(text_or(Some(""), &fallback), fallback);
  assert_eq!(text_or(None, &fallback), fallback);
}
