//! Claude Code and Codex output shapes for each normalized decision.

use serde_json::json;

use super::output;
use crate::encode::Normal;
use crate::encode::tests::parsed;
use crate::event::HostEvent;

#[test]
fn allow_is_silent() {
  assert_eq!(output("PreToolUse", HostEvent::ToolPre, Normal::Allow), "");
}

#[test]
fn a_pre_tool_use_deny_or_ask_is_a_permission_decision() {
  let decide = |normal, kind: &str| {
    let out = output("PreToolUse", HostEvent::ShellPre, normal);
    assert_eq!(
      parsed(&out),
      json!({ "hookSpecificOutput": { "hookEventName": "PreToolUse",
        "permissionDecision": kind, "permissionDecisionReason": "protected file" } })
    );
  };
  decide(Normal::Deny("protected file"), "deny");
  decide(Normal::Ask("protected file"), "ask");
}

#[test]
fn advice_is_additional_context_where_the_event_takes_it_else_a_system_message() {
  for (native, event) in [
    ("PreToolUse", HostEvent::ToolPre),
    ("PostToolUse", HostEvent::ToolPost),
    ("SessionStart", HostEvent::SessionStart),
    ("UserPromptSubmit", HostEvent::Prompt),
  ] {
    let out = output(native, event, Normal::Advisory("run the tests"));
    assert_eq!(
      parsed(&out),
      json!({ "hookSpecificOutput": { "hookEventName": native,
        "additionalContext": "run the tests" } })
    );
  }
  for (native, event) in [
    ("PreCompact", HostEvent::PreCompact),
    ("SessionEnd", HostEvent::SessionUnload),
    ("PermissionRequest", HostEvent::PermissionEvaluate),
  ] {
    let out = output(native, event, Normal::Advisory("run the tests"));
    assert_eq!(parsed(&out), json!({ "systemMessage": "run the tests" }));
  }
}

#[test]
fn a_block_and_a_prompt_deny_are_decision_block() {
  let block = json!({ "decision": "block", "reason": "lint failed" });
  let post = output(
    "PostToolUse",
    HostEvent::ToolPost,
    Normal::Block("lint failed"),
  );
  assert_eq!(parsed(&post), block);
  let prompt = output(
    "UserPromptSubmit",
    HostEvent::Prompt,
    Normal::Deny("lint failed"),
  );
  assert_eq!(parsed(&prompt), block);
}

#[test]
fn permission_request_denies_with_behavior_and_leaves_ask_to_the_host() {
  let out = output(
    "PermissionRequest",
    HostEvent::PermissionEvaluate,
    Normal::Deny("protected file"),
  );
  assert_eq!(
    parsed(&out),
    json!({ "hookSpecificOutput": { "hookEventName": "PermissionRequest",
      "decision": { "behavior": "deny", "message": "protected file" } } })
  );
  let ask = output(
    "PermissionRequest",
    HostEvent::PermissionEvaluate,
    Normal::Ask("confirm"),
  );
  assert_eq!(ask, "");
}

#[test]
fn keys_keep_the_typescript_order() {
  let out = output("PreToolUse", HostEvent::ToolPre, Normal::Deny("no"));
  assert_eq!(
    out,
    "{\"hookSpecificOutput\":{\"hookEventName\":\"PreToolUse\",\"permissionDecision\":\"deny\",\
     \"permissionDecisionReason\":\"no\"}}\n"
  );
}
