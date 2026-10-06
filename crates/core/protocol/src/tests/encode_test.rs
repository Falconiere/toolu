//! The cases of `packages/toolu-core/src/host/__tests__/host-encode.test.ts`.

use serde_json::{Value, json};

use super::{EncodeError, Encoded, degrade_ask, encode, supports_ask};
use crate::decision::{Decision, GateClass};
use crate::event::HostEvent::{
  self, PermissionEvaluate, PreCompact, Prompt, SessionStart, SessionUnload, ShellPre, ToolPost,
  ToolPre,
};
use crate::host::Host;
use crate::native::native_event;
use crate::text::Text;

fn text(value: &str) -> Text {
  Text::new(value).unwrap()
}

/// The six decisions of `host-encode.test.ts`.
fn decision(kind: &str) -> Decision {
  let doc = match kind {
    "allow" => json!({ "kind": "allow" }),
    "ask" => json!({ "kind": "ask", "reason": "confirm .env write" }),
    "deny" => json!({ "kind": "deny", "reason": "protected file" }),
    "advise" => json!({ "kind": "advisory", "message": "run the tests" }),
    "post_block" => json!({ "kind": "post_block", "reason": "lint failed" }),
    "failure" => json!({ "kind": "runtime_failure", "reason": "gate crashed", "code": "nonzero" }),
    other => panic!("no decision {other}"),
  };
  serde_json::from_value(doc).unwrap()
}

fn allow() -> Decision {
  decision("allow")
}
fn ask() -> Decision {
  decision("ask")
}
fn deny() -> Decision {
  decision("deny")
}
fn advise() -> Decision {
  decision("advise")
}
fn post_block() -> Decision {
  decision("post_block")
}
fn failure() -> Decision {
  decision("failure")
}

/// The command stdout, parsed; `""` for silent success.
fn stdout(host: Host, event: HostEvent, decision: &Decision) -> Value {
  let Ok(Encoded::Command(out)) = encode(host, event, decision) else {
    panic!("expected command output for {host:?} {event:?}");
  };
  if out.is_empty() {
    return json!("");
  }
  assert!(out.ends_with('\n'), "{out:?}");
  serde_json::from_str(&out).unwrap()
}

#[test]
fn supports_ask_answers_the_pre_tool_use_question() {
  assert!(supports_ask(Host::Claude, ToolPre));
  assert!(!supports_ask(Host::Codex, ToolPre));
  assert!(!supports_ask(Host::Cursor, ToolPre));
  assert!(!supports_ask(Host::Hermes, ToolPre));
  assert!(!supports_ask(Host::Opencode, ToolPre));
}

#[test]
fn cursor_asks_only_before_shell_execution_and_permission_requests_defer() {
  assert!(supports_ask(Host::Cursor, ShellPre));
  assert!(supports_ask(Host::Codex, PermissionEvaluate));
  assert!(!supports_ask(Host::Claude, Prompt));
  assert!(!supports_ask(Host::Opencode, ToolPost));
}

#[test]
fn a_guardrail_ask_becomes_a_deny_where_the_host_cannot_prompt() {
  let deny = Decision::Deny {
    reason: text("confirm .env write"),
  };
  assert_eq!(
    degrade_ask(Host::Codex, ToolPre, ask(), GateClass::Guardrail),
    deny
  );
  assert_eq!(
    degrade_ask(Host::Opencode, ToolPre, ask(), GateClass::Guardrail),
    deny
  );
}

#[test]
fn a_judgement_ask_becomes_advice_where_the_host_cannot_prompt() {
  let advice = Decision::Advisory {
    message: text("confirm .env write"),
  };
  assert_eq!(
    degrade_ask(Host::Hermes, ShellPre, ask(), GateClass::Judgement),
    advice
  );
  assert_eq!(
    degrade_ask(Host::Opencode, ToolPre, ask(), GateClass::Judgement),
    advice
  );
}

#[test]
fn ask_survives_where_the_host_prompts_and_other_decisions_pass_through() {
  assert_eq!(
    degrade_ask(Host::Claude, ToolPre, ask(), GateClass::Guardrail),
    ask()
  );
  assert_eq!(
    degrade_ask(Host::Cursor, ShellPre, ask(), GateClass::Judgement),
    ask()
  );
  assert_eq!(
    degrade_ask(Host::Codex, ToolPre, deny(), GateClass::Judgement),
    deny()
  );
}

#[test]
fn allow_is_silent_success_on_every_claude_and_codex_event() {
  for host in [Host::Claude, Host::Codex] {
    for event in HostEvent::ALL {
      assert_eq!(
        stdout(host, event, &allow()),
        json!(""),
        "{host:?} {event:?}"
      );
    }
  }
}

#[test]
fn codex_never_receives_ask_on_pre_tool_use_it_fails_closed_to_deny() {
  assert_eq!(
    stdout(Host::Codex, ToolPre, &ask()),
    json!({ "hookSpecificOutput": { "hookEventName": "PreToolUse",
      "permissionDecision": "deny", "permissionDecisionReason": "confirm .env write" } })
  );
}

#[test]
fn post_tool_use_blocks_with_decision_block_and_advises_with_context() {
  let block = |reason: &str| json!({ "decision": "block", "reason": reason });
  assert_eq!(
    stdout(Host::Codex, ToolPost, &post_block()),
    block("lint failed")
  );
  assert_eq!(
    stdout(Host::Claude, ToolPost, &deny()),
    block("protected file")
  );
  assert_eq!(
    stdout(Host::Claude, ToolPost, &advise()),
    json!({ "hookSpecificOutput": { "hookEventName": "PostToolUse",
      "additionalContext": "run the tests" } })
  );
}

#[test]
fn context_only_events_cannot_block_so_deny_degrades_to_context() {
  assert_eq!(
    stdout(Host::Claude, SessionStart, &deny()),
    json!({ "hookSpecificOutput": { "hookEventName": "SessionStart",
      "additionalContext": "protected file" } })
  );
  assert_eq!(
    stdout(Host::Claude, PreCompact, &advise()),
    json!({ "systemMessage": "run the tests" })
  );
  assert_eq!(
    stdout(Host::Codex, SessionUnload, &post_block()),
    json!({ "systemMessage": "lint failed" })
  );
}

#[test]
fn a_runtime_failure_encodes_as_a_deny_on_blocking_events() {
  let crashed = Decision::Deny {
    reason: text("gate crashed"),
  };
  for host in Host::ALL {
    for event in [ToolPre, ShellPre, PermissionEvaluate] {
      if native_event(host, event).is_none() {
        continue;
      }
      assert_eq!(
        encode(host, event, &failure()),
        encode(host, event, &crashed),
        "{host:?} {event:?}"
      );
    }
  }
}

#[test]
fn a_runtime_failure_on_a_non_blocking_event_is_advice() {
  let advice = Decision::Advisory {
    message: text("gate crashed"),
  };
  for host in Host::ALL {
    for event in [ToolPost, Prompt, SessionStart, SessionUnload, PreCompact] {
      if native_event(host, event).is_none() {
        continue;
      }
      assert_eq!(
        encode(host, event, &failure()),
        encode(host, event, &advice),
        "{host:?} {event:?}"
      );
    }
  }
}

#[test]
fn a_post_block_before_an_action_is_a_deny() {
  for host in Host::ALL {
    for event in [ToolPre, ShellPre] {
      assert_eq!(
        encode(host, event, &post_block()),
        encode(
          host,
          event,
          &Decision::Deny {
            reason: text("lint failed"),
          }
        )
      );
    }
  }
}

#[test]
fn ask_never_reaches_a_host_or_event_that_cannot_prompt() {
  let decisions = [allow(), ask(), deny(), advise(), post_block(), failure()];
  for host in Host::ALL {
    for event in HostEvent::ALL {
      if native_event(host, event).is_none() || supports_ask(host, event) {
        continue;
      }
      for decision in &decisions {
        let out = match encode(host, event, decision).unwrap() {
          Encoded::Command(out) => out,
          Encoded::Callback(callback) => callback.json(),
        };
        assert!(!out.contains("\"ask\""), "{host:?} {event:?} {out}");
      }
    }
  }
}

#[test]
fn an_event_the_host_does_not_have_is_a_wiring_error() {
  let err = encode(Host::Cursor, PermissionEvaluate, &deny()).unwrap_err();
  assert_eq!(
    err,
    EncodeError {
      host: Host::Cursor,
      event: PermissionEvaluate
    }
  );
  assert_eq!(
    err.to_string(),
    "cursor has no native event for permission/evaluate"
  );
  assert!(encode(Host::Hermes, PreCompact, &allow()).is_err());
  assert!(encode(Host::Opencode, PermissionEvaluate, &deny()).is_err());
}

#[test]
fn cursor_pre_tool_use_does_not_enforce_ask_so_ask_fails_closed_to_deny() {
  assert_eq!(
    stdout(Host::Cursor, ToolPre, &ask()),
    json!({ "permission": "deny", "user_message": "confirm .env write",
      "agent_message": "confirm .env write" })
  );
  assert_eq!(
    stdout(Host::Cursor, ShellPre, &ask()),
    json!({ "permission": "ask", "user_message": "confirm .env write",
      "agent_message": "confirm .env write" })
  );
}
