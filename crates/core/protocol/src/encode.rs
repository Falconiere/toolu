//! Per-host output encoders (`packages/toolu-core/src/host/host-encode.ts`): one
//! [`Decision`] in, the output each host's hook contract accepts out.
//! Normalization runs first and fails closed, so a caller's mistake (an
//! undegraded `ask`, a runtime failure) never becomes a silent allow.
//!
//! `ask` degradation follows `gate-mode.sh`: where the host cannot prompt, a
//! security guardrail's ask becomes a deny and a judgement gate's becomes advice.
//! Every line is `JSON.stringify(value) + "\n"`: keys in the order TypeScript
//! writes them, strings escaped by `serde_json`.

use std::fmt;

use crate::decision::{Decision, GateClass};
use crate::event::HostEvent;
use crate::host::Host;
use crate::native::native_event;
use crate::text::Text;

mod cursor;
mod hermes;
mod hook;
mod object;
mod opencode;

use object::Object;

/// What a host gets back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Encoded {
  /// A command hook's exact stdout (exit 0, no stderr); `""` is silent success.
  Command(String),
  /// An in-process `OpenCode` callback.
  Callback(Callback),
}

/// An `OpenCode` before or after hook: continue, or throw to refuse the call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Callback {
  /// Continue or throw.
  pub action: CallbackAction,
  /// The refusal, or the advice appended to the result.
  pub message: Option<String>,
}

/// What an `OpenCode` hook does with the call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallbackAction {
  /// Let the call run.
  Continue,
  /// Refuse it.
  Throw,
}

/// A decision for an event the host does not have: a wiring error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EncodeError {
  /// The host.
  pub host: Host,
  /// The event it lacks.
  pub event: HostEvent,
}

/// A decision after normalization: only what the event can carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Normal<'a> {
  Allow,
  Ask(&'a str),
  Deny(&'a str),
  Advisory(&'a str),
  Block(&'a str),
}

/// Can `host` put an `ask` for `event` in front of a human?
pub fn supports_ask(host: Host, event: HostEvent) -> bool {
  match host {
    Host::Claude => pre_action(event) || event == HostEvent::PermissionEvaluate,
    // Codex drops a PreToolUse ask and runs the tool; PermissionRequest shows its own prompt.
    Host::Codex => event == HostEvent::PermissionEvaluate,
    // Cursor accepts but does not enforce ask on preToolUse.
    Host::Cursor => event == HostEvent::ShellPre,
    Host::Hermes | Host::Opencode => false,
  }
}

/// Degrade `ask` where the host cannot prompt: guardrail to deny, judgement to advice.
pub fn degrade_ask(host: Host, event: HostEvent, decision: Decision, class: GateClass) -> Decision {
  if supports_ask(host, event) {
    return decision;
  }
  let Decision::Ask { reason } = decision else {
    return decision;
  };
  match class {
    GateClass::Guardrail => Decision::Deny { reason },
    GateClass::Judgement => Decision::Advisory { message: reason },
  }
}

/// Encode `decision` for `event` in `host`'s native hook output.
///
/// # Errors
/// [`EncodeError`] when `host` has no native event for `event`.
pub fn encode(host: Host, event: HostEvent, decision: &Decision) -> Result<Encoded, EncodeError> {
  let native = native_event(host, event).ok_or(EncodeError { host, event })?;
  let normal = normalize(host, event, decision);
  Ok(match host {
    Host::Claude | Host::Codex => Encoded::Command(hook::output(native, event, normal)),
    Host::Cursor => Encoded::Command(cursor::output(event, normal)),
    Host::Hermes => Encoded::Command(hermes::output(event, normal)),
    Host::Opencode => Encoded::Callback(opencode::callback(normal)),
  })
}

/// The events before an action.
fn pre_action(event: HostEvent) -> bool {
  matches!(event, HostEvent::ToolPre | HostEvent::ShellPre)
}

/// The events whose hook can refuse.
fn blocking(event: HostEvent) -> bool {
  pre_action(event) || event == HostEvent::PermissionEvaluate
}

/// The events that only take context.
fn context_only(event: HostEvent) -> bool {
  matches!(
    event,
    HostEvent::SessionStart | HostEvent::SessionUnload | HostEvent::PreCompact
  )
}

fn normalize(host: Host, event: HostEvent, decision: &Decision) -> Normal<'_> {
  match decision {
    Decision::Allow => Normal::Allow,
    Decision::Advisory { message } => Normal::Advisory(message.as_str()),
    Decision::Ask { reason } if supports_ask(host, event) => Normal::Ask(reason.as_str()),
    Decision::Ask { reason } => refuse_or_advise(pre_action(event), reason),
    Decision::RuntimeFailure { reason, .. } => refuse_or_advise(blocking(event), reason),
    Decision::Deny { reason } | Decision::Block { reason } if event == HostEvent::ToolPost => {
      Normal::Block(reason.as_str())
    }
    Decision::Deny { reason } | Decision::Block { reason } => {
      refuse_or_advise(!context_only(event), reason)
    }
  }
}

/// A deny when the event can refuse, else advice.
fn refuse_or_advise(refuse: bool, reason: &Text) -> Normal<'_> {
  if refuse {
    Normal::Deny(reason.as_str())
  } else {
    Normal::Advisory(reason.as_str())
  }
}

impl<'a> Normal<'a> {
  /// The advice or reason; `""` for allow.
  fn text(self) -> &'a str {
    match self {
      Normal::Allow => "",
      Normal::Ask(text) | Normal::Deny(text) | Normal::Advisory(text) | Normal::Block(text) => text,
    }
  }
}

impl fmt::Display for EncodeError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(
      f,
      "{} has no native event for {}",
      self.host.name(),
      self.event.slug()
    )
  }
}

impl std::error::Error for EncodeError {}

#[cfg(test)]
#[path = "tests/encode_test.rs"]
mod tests;
