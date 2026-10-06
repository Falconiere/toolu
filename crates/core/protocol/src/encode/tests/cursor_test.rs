//! Cursor output shapes for each normalized decision.

use serde_json::{Value, json};

use super::output;
use crate::encode::Normal;
use crate::event::HostEvent;

fn parsed(out: &str) -> Value {
  assert!(out.ends_with('\n'), "{out:?}");
  serde_json::from_str(out).unwrap()
}

#[test]
fn permission_events_always_answer_with_a_permission() {
  let pre = |normal| parsed(&output(HostEvent::ToolPre, normal));
  assert_eq!(pre(Normal::Allow), json!({ "permission": "allow" }));
  assert_eq!(
    pre(Normal::Advisory("run the tests")),
    json!({ "permission": "allow", "agent_message": "run the tests" })
  );
  assert_eq!(
    parsed(&output(HostEvent::ShellPre, Normal::Deny("protected file"))),
    json!({ "permission": "deny", "user_message": "protected file",
      "agent_message": "protected file" })
  );
  assert_eq!(
    parsed(&output(HostEvent::ShellPre, Normal::Ask("confirm"))),
    json!({ "permission": "ask", "user_message": "confirm", "agent_message": "confirm" })
  );
}

#[test]
fn prompts_continue_or_stop() {
  assert_eq!(
    parsed(&output(HostEvent::Prompt, Normal::Deny("protected file"))),
    json!({ "continue": false, "user_message": "protected file" })
  );
  assert_eq!(
    parsed(&output(HostEvent::Prompt, Normal::Advisory("x"))),
    json!({ "continue": true })
  );
}

#[test]
fn post_and_session_hooks_take_additional_context_and_the_rest_get_an_empty_object() {
  assert_eq!(
    parsed(&output(HostEvent::ToolPost, Normal::Block("lint failed"))),
    json!({ "additional_context": "lint failed" })
  );
  assert_eq!(
    parsed(&output(
      HostEvent::SessionStart,
      Normal::Advisory("run the tests")
    )),
    json!({ "additional_context": "run the tests" })
  );
  assert_eq!(output(HostEvent::PreCompact, Normal::Advisory("x")), "{}\n");
  assert_eq!(output(HostEvent::SessionUnload, Normal::Allow), "{}\n");
}

#[test]
fn keys_keep_the_typescript_order() {
  assert_eq!(
    output(HostEvent::ShellPre, Normal::Deny("no")),
    "{\"permission\":\"deny\",\"user_message\":\"no\",\"agent_message\":\"no\"}\n"
  );
}
