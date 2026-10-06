//! Claude Code and Codex output: `hookSpecificOutput`, silent success on allow.

use super::{Normal, Object};
use crate::event::HostEvent;

/// Claude/Codex events whose `hookSpecificOutput` carries `additionalContext`.
fn takes_context(event: HostEvent) -> bool {
  matches!(
    event,
    HostEvent::ToolPre
      | HostEvent::ShellPre
      | HostEvent::ToolPost
      | HostEvent::SessionStart
      | HostEvent::Prompt
  )
}

/// The stdout for `normal` on `event`, whose native name is `native`.
pub(super) fn output(native: &str, event: HostEvent, normal: Normal<'_>) -> String {
  match normal {
    Normal::Allow => String::new(),
    Normal::Block(reason) => block(reason),
    Normal::Advisory(message) if takes_context(event) => Object::new()
      .object(
        "hookSpecificOutput",
        Object::new()
          .text("hookEventName", native)
          .text("additionalContext", message),
      )
      .line(),
    Normal::Advisory(message) => Object::new().text("systemMessage", message).line(),
    Normal::Deny(reason) | Normal::Ask(reason) => refuse(native, event, normal, reason),
  }
}

/// A deny or ask: `PermissionRequest` answers with `decision.behavior` and leaves
/// an ask to the host's own prompt, a prompt blocks, and a tool call gets a
/// `permissionDecision`.
fn refuse(native: &str, event: HostEvent, normal: Normal<'_>, reason: &str) -> String {
  let deny = matches!(normal, Normal::Deny(_));
  if event == HostEvent::PermissionEvaluate {
    if !deny {
      return String::new();
    }
    let decision = Object::new()
      .text("behavior", "deny")
      .text("message", reason);
    return Object::new()
      .object(
        "hookSpecificOutput",
        Object::new()
          .text("hookEventName", native)
          .object("decision", decision),
      )
      .line();
  }
  if event == HostEvent::Prompt {
    return block(reason);
  }
  let specific = Object::new()
    .text("hookEventName", native)
    .text("permissionDecision", if deny { "deny" } else { "ask" })
    .text("permissionDecisionReason", reason);
  Object::new().object("hookSpecificOutput", specific).line()
}

fn block(reason: &str) -> String {
  Object::new()
    .text("decision", "block")
    .text("reason", reason)
    .line()
}

#[cfg(test)]
#[path = "tests/hook_test.rs"]
mod tests;
