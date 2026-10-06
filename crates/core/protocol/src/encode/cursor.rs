//! Cursor output: permission events always name a `permission`, and an empty
//! reply blocks, so every event writes an object.

use super::{Normal, Object, pre_action};
use crate::event::HostEvent;

/// The stdout for `normal` on `event`.
pub(super) fn output(event: HostEvent, normal: Normal<'_>) -> String {
  if pre_action(event) {
    return permission(normal);
  }
  if event == HostEvent::Prompt {
    return match normal {
      Normal::Deny(reason) => Object::new()
        .flag("continue", false)
        .text("user_message", reason)
        .line(),
      Normal::Allow | Normal::Ask(_) | Normal::Advisory(_) | Normal::Block(_) => {
        Object::new().flag("continue", true).line()
      }
    };
  }
  let context = matches!(normal, Normal::Advisory(_) | Normal::Block(_));
  if context && matches!(event, HostEvent::ToolPost | HostEvent::SessionStart) {
    return Object::new()
      .text("additional_context", normal.text())
      .line();
  }
  Object::new().line()
}

fn permission(normal: Normal<'_>) -> String {
  match normal {
    Normal::Deny(reason) | Normal::Ask(reason) => {
      let kind = if matches!(normal, Normal::Deny(_)) {
        "deny"
      } else {
        "ask"
      };
      Object::new()
        .text("permission", kind)
        .text("user_message", reason)
        .text("agent_message", reason)
        .line()
    }
    Normal::Advisory(message) => Object::new()
      .text("permission", "allow")
      .text("agent_message", message)
      .line(),
    Normal::Allow | Normal::Block(_) => Object::new().text("permission", "allow").line(),
  }
}

#[cfg(test)]
#[path = "tests/cursor_test.rs"]
mod tests;
