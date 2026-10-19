//! One module walk over one payload (`dispatchModules` in `dispatch-walk.ts`):
//! the built-in gates in table order, each folded as it decides, then the
//! registry phase, whose results are folded in order once it ends.

use toolu_protocol::decision::Decision;
use toolu_protocol::encode::{Encoded, encode};
use toolu_protocol::event::HostEvent;
use toolu_protocol::host::Host;

use super::event::{Payload, View, host_event};
use super::fold::{Folded, WalkState, event_name};
use super::output::substituted;
use super::session::Session;
use super::{ModuleResult, Phase};
use toolu_runtime::registry::rule::RuleContext;

use crate::gate::Gate;
use crate::registry::phase::{RegistryWalk, run_registry};
use crate::trace::{Step, StepKind, StepStatus};

/// A decision as the hook JSON a bash module would have printed: Codex output
/// for Codex, Claude output for Claude-shaped hosts. Muse never reaches it
/// until #545 ports the walk.
pub(crate) fn encoded(host: Host, event: HostEvent, decision: &Decision) -> String {
  let target = match host {
    Host::Codex | Host::Muse => host,
    Host::Claude | Host::Cursor | Host::Hermes => Host::Claude,
  };
  match encode(target, event, decision) {
    Ok(Encoded::Command(stdout)) => substituted(&stdout).to_owned(),
    Err(_) => String::new(),
  }
}

/// The dispatcher's own failure, which blocks like `hook-main.ts` does.
fn failed(phase: Phase, message: &str) -> ModuleResult {
  let prefix = if phase == Phase::Pre { "blocked: " } else { "" };
  ModuleResult {
    stdout: String::new(),
    stderr: format!(
      "{prefix}toolu {} dispatcher failed: {message}\n",
      event_name(phase)
    ),
    exit_code: 2,
  }
}

/// One built-in's result: its decision encoded, or exit 1 when it failed. Its
/// warnings go to `stderr` first, as TypeScript prints them while it runs.
fn run_gate(
  gate: &dyn Gate,
  (view, ctx): (&View, &RuleContext<'_>),
  (host, event): (Host, HostEvent),
  stderr: &mut String,
  trace: &mut Vec<Step>,
) -> Folded {
  let mut warnings = Vec::new();
  let decided = gate.run_warning(&view.event, ctx, &mut warnings);
  for warning in warnings {
    stderr.push_str(&warning);
    stderr.push('\n');
  }
  let (result, status) = match decided {
    Ok(decision) => (
      ModuleResult {
        stdout: encoded(host, event, &decision),
        ..ModuleResult::default()
      },
      StepStatus::Decided,
    ),
    Err(message) => (
      ModuleResult {
        exit_code: 1,
        ..ModuleResult::default()
      },
      StepStatus::Failed(message),
    ),
  };
  trace.push(Step {
    module: gate.name().to_owned(),
    kind: StepKind::Builtin,
    status,
  });
  Folded {
    name: gate.name().to_owned(),
    result,
    truncated: false,
  }
}

/// Walk `payload`: built-ins, then the registry.
pub(crate) fn walk(
  payload: &Payload,
  session: &Session<'_>,
  trace: &mut Vec<Step>,
) -> ModuleResult {
  // The fold reads Claude-shaped module output; #545 ports the walk to Muse.
  if session.host == Host::Muse {
    return failed(session.phase, "Muse tool calls are not gated yet (#545)");
  }
  let view = match View::of(payload, session) {
    Ok(view) => view,
    Err(message) => return failed(session.phase, &message),
  };
  let ctx = view.rule_context(payload, session);
  let event = host_event(&view.event);
  let mut state = WalkState::new(session.phase);
  for gate in session.options.builtins {
    let folded = run_gate(
      *gate,
      (&view, &ctx),
      (session.host, event),
      &mut state.stderr,
      trace,
    );
    if let Some(done) = state.consume(&folded) {
      return done;
    }
  }
  let registry = RegistryWalk {
    payload,
    session,
    view: &view,
    ctx: &ctx,
    event,
  };
  for folded in run_registry(&registry, &mut state.stderr, trace) {
    if let Some(done) = state.consume(&folded) {
      return done;
    }
  }
  state.settle()
}

#[cfg(test)]
#[path = "tests/walk_test.rs"]
mod tests;
