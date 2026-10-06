//! Hermes output: only `pre_tool_call` blocks, and only `pre_llm_call` takes context.

use super::{Normal, Object, pre_action};
use crate::event::HostEvent;

/// The stdout for `normal` on `event`.
pub(super) fn output(event: HostEvent, normal: Normal<'_>) -> String {
  let deny = matches!(normal, Normal::Deny(_));
  if pre_action(event) && deny {
    return Object::new()
      .text("action", "block")
      .text("message", normal.text())
      .line();
  }
  if event == HostEvent::Prompt && (deny || matches!(normal, Normal::Advisory(_))) {
    return Object::new().text("context", normal.text()).line();
  }
  String::new()
}

#[cfg(test)]
#[path = "tests/hermes_test.rs"]
mod tests;
