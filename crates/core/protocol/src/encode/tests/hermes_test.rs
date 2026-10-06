//! Hermes output shapes for each normalized decision.

use serde_json::{Value, json};

use super::output;
use crate::encode::Normal;
use crate::event::HostEvent;

fn parsed(out: &str) -> Value {
  if out.is_empty() {
    return json!("");
  }
  serde_json::from_str(out).unwrap()
}

#[test]
fn pre_tool_call_blocks_with_action_block_and_allow_and_advice_are_silent() {
  let pre = |normal| parsed(&output(HostEvent::ToolPre, normal));
  assert_eq!(
    pre(Normal::Deny("protected file")),
    json!({ "action": "block", "message": "protected file" })
  );
  assert_eq!(pre(Normal::Advisory("run the tests")), json!(""));
  assert_eq!(pre(Normal::Allow), json!(""));
}

#[test]
fn pre_llm_call_takes_context_and_tool_and_session_hooks_have_no_channel() {
  assert_eq!(
    parsed(&output(
      HostEvent::Prompt,
      Normal::Advisory("run the tests")
    )),
    json!({ "context": "run the tests" })
  );
  assert_eq!(
    parsed(&output(HostEvent::Prompt, Normal::Deny("protected file"))),
    json!({ "context": "protected file" })
  );
  assert_eq!(
    output(HostEvent::ToolPost, Normal::Block("lint failed")),
    ""
  );
  assert_eq!(output(HostEvent::SessionStart, Normal::Advisory("x")), "");
  assert_eq!(output(HostEvent::Prompt, Normal::Allow), "");
}
