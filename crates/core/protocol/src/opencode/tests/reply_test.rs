use serde_json::{Value, json};

use super::{Ran, Reply};
use crate::opencode::Hook;

fn ran<'a>(exit_code: i32, stdout: &'a str, stderr: &'a str) -> Ran<'a> {
  Ran {
    label: "toolu plugin: toolu hook pre-tools on PreToolUse",
    exit_code,
    stdout,
    stderr,
    timed_out: false,
  }
}

fn folded(hook: Hook, runs: &[Ran<'_>]) -> Value {
  let mut reply = Reply::default();
  for one in runs {
    reply.add(hook, one);
  }
  serde_json::from_str(&reply.json()).unwrap()
}

#[test]
fn silence_continues() {
  assert_eq!(
    folded(Hook::ToolBefore, &[ran(0, "", "")]),
    json!({"action": "continue"})
  );
  assert_eq!(folded(Hook::ToolBefore, &[]), json!({"action": "continue"}));
}

#[test]
fn a_pre_tool_use_deny_or_ask_or_block_refuses_the_call() {
  let deny = r#"{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"protected file"}}"#;
  assert_eq!(
    folded(Hook::ToolBefore, &[ran(0, deny, "")]),
    json!({"action": "deny", "reason": "protected file"})
  );
  let ask = r#"{"hookSpecificOutput":{"permissionDecision":"ask"}}"#;
  assert_eq!(
    folded(Hook::ToolBefore, &[ran(0, ask, "")])["action"],
    "deny"
  );
  let block = r#"{"decision":"block","reason":"lint failed"}"#;
  assert_eq!(
    folded(Hook::ToolBefore, &[ran(0, block, "")]),
    json!({"action": "deny", "reason": "lint failed"})
  );
  let stop = r#"{"continue":false}"#;
  assert_eq!(
    folded(Hook::ToolBefore, &[ran(0, stop, "")]),
    json!({"action": "deny", "reason": "stopped by a toolu hook"})
  );
}

#[test]
fn exit_2_refuses_with_stderr_without_the_launcher_prefix() {
  let reply = folded(Hook::ToolBefore, &[ran(2, "", "blocked: no push\n")]);
  assert_eq!(reply, json!({"action": "deny", "reason": "no push"}));
  let silent = folded(Hook::ToolBefore, &[ran(2, "", "")]);
  assert_eq!(
    silent["reason"],
    "toolu plugin: toolu hook pre-tools on PreToolUse refused without a reason"
  );
}

#[test]
fn after_a_tool_a_refusal_blocks_with_its_reason_and_keeps_context() {
  let block = r#"{"decision":"block","reason":"3 lines over"}"#;
  let advice = r#"{"hookSpecificOutput":{"additionalContext":"run the tests"}}"#;
  assert_eq!(
    folded(Hook::ToolAfter, &[ran(0, block, ""), ran(0, advice, "")]),
    json!({"action": "block", "reason": "3 lines over", "context": "run the tests"})
  );
  assert_eq!(
    folded(Hook::ToolAfter, &[ran(2, "", "check failed")]),
    json!({"action": "block", "reason": "check failed"})
  );
}

#[test]
fn session_output_is_context_and_messages() {
  let start = r#"{"systemMessage":"Toolu is on!","hookSpecificOutput":{"hookEventName":"SessionStart","additionalContext":"use the ledger"}}"#;
  assert_eq!(
    folded(
      Hook::SessionStart,
      &[ran(0, start, ""), ran(0, "plain text\n", "")]
    ),
    json!({"action": "continue", "context": "use the ledger\n\nplain text",
      "message": "Toolu is on!"})
  );
  let block = r#"{"decision":"block","reason":"secret in prompt"}"#;
  assert_eq!(
    folded(Hook::Prompt, &[ran(0, block, "")]),
    json!({"action": "continue", "context": "secret in prompt"})
  );
  assert_eq!(
    folded(Hook::ToolBefore, &[ran(0, "plain text", "")]),
    json!({"action": "continue"})
  );
}

#[test]
fn an_abnormal_status_or_timeout_fails_closed_only_before_a_tool() {
  let crashed = ran(101, "", "panicked\n");
  assert_eq!(
    folded(Hook::ToolBefore, std::slice::from_ref(&crashed)),
    json!({"action": "deny",
      "reason": "toolu plugin: toolu hook pre-tools on PreToolUse ended with status 101: panicked"})
  );
  assert_eq!(
    folded(Hook::SessionStart, &[ran(1, "", "")]),
    json!({"action": "continue",
      "message": "toolu plugin: toolu hook pre-tools on PreToolUse ended with status 1"})
  );
  let slow = Ran {
    timed_out: true,
    ..ran(0, "", "")
  };
  let mut reply = Reply::default();
  reply.add(Hook::ToolBefore, &slow);
  assert!(reply.denied());
  assert!(reply.json().contains("timed out"));
}

#[test]
fn a_bridge_failure_denies_before_a_tool_and_is_a_message_elsewhere() {
  let mut before = Reply::default();
  before.fail(Hook::ToolBefore, "no hooks.json".to_owned());
  assert_eq!(
    serde_json::from_str::<Value>(&before.json()).unwrap(),
    json!({"action": "deny", "reason": "no hooks.json"})
  );
  let mut prompt = Reply::default();
  prompt.fail(Hook::Prompt, "no hooks.json".to_owned());
  assert!(!prompt.denied());
  assert!(prompt.json().contains("\"message\":\"no hooks.json\""));
}
