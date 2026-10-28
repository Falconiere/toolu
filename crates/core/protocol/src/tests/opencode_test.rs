use super::Hook;
use crate::decision::{Decision, GateClass};
use crate::encode::{degrade_ask, encode, supports_ask};
use crate::event::HostEvent;
use crate::host::Host;
use crate::text::Text;

#[test]
fn every_hook_name_parses_back_and_runs_its_claude_event() {
  let names: Vec<&str> = Hook::ALL.iter().map(|hook| hook.name()).collect();
  assert_eq!(
    names,
    [
      "session.start",
      "session.prompt",
      "session.compaction",
      "tool.execute.before",
      "tool.execute.after"
    ]
  );
  for hook in Hook::ALL {
    assert_eq!(Hook::parse(hook.name()), Some(hook));
    assert_eq!(Hook::for_claude_event(hook.claude_event()), Some(hook));
  }
  assert_eq!(Hook::for_claude_event("PreToolUse"), Some(Hook::ToolBefore));
  assert_eq!(
    Hook::for_claude_event("UserPromptSubmit"),
    Some(Hook::Prompt)
  );
}

#[test]
fn events_opencode_lacks_and_unknown_names_are_none() {
  for name in ["SessionEnd", "PermissionRequest", "Stop", ""] {
    assert_eq!(Hook::for_claude_event(name), None, "{name}");
  }
  for text in ["tool.execute", "PreToolUse", "session.Start", ""] {
    assert_eq!(Hook::parse(text), None, "{text}");
  }
}

#[test]
fn only_the_before_tool_hook_is_enforcing() {
  let enforcing: Vec<Hook> = Hook::ALL
    .into_iter()
    .filter(|hook| hook.enforcing())
    .collect();
  assert_eq!(enforcing, [Hook::ToolBefore]);
}

#[test]
fn opencode_never_asks_and_hooks_write_claude_shapes_for_the_bridge() {
  let reason = |text: &str| Text::new(text).unwrap();
  let deny = Decision::Deny {
    reason: reason("protected file"),
  };
  for event in HostEvent::ALL {
    assert!(!supports_ask(Host::Opencode, event), "{event:?}");
    assert_eq!(
      encode(Host::Opencode, event, &deny),
      encode(Host::Claude, event, &deny),
      "{event:?}"
    );
  }
  let ask = Decision::Ask {
    reason: reason("confirm .env write"),
  };
  let degraded = degrade_ask(
    Host::Opencode,
    HostEvent::ToolPre,
    ask,
    GateClass::Guardrail,
  );
  assert!(matches!(degraded, Decision::Deny { .. }), "{degraded:?}");
}
