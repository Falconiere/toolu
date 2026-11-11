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
use std::cell::RefCell;
use std::path::Path;
use toolu_runtime::registry::rule::RuleContext;

use crate::gate::Gate;
use crate::quality::shell;
use crate::quality::{QualityBatch, QualityBinding};
use crate::registry::phase::{RegistryPlan, RegistryWalk, run_registry};
use crate::trace::{Step, StepKind, StepStatus};

/// A decision as the hook JSON a bash module would have printed: Codex output
/// for Codex, Claude output for Claude-shaped hosts, and native Muse output.
pub(crate) fn encoded(host: Host, event: HostEvent, decision: &Decision) -> String {
  let target = match host {
    Host::Codex | Host::Muse => host,
    Host::Claude | Host::Cursor | Host::Hermes | Host::Opencode => Host::Claude,
  };
  match encode(target, event, decision) {
    Ok(Encoded::Command(stdout)) => substituted(&stdout).to_owned(),
    Err(_) => String::new(),
  }
}

/// The dispatcher's own failure, which blocks like `hook-main.ts` does.
pub(super) fn failed(phase: Phase, message: &str) -> ModuleResult {
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

fn shell_decisions(
  selected: &mut shell::Selection,
  (active, bindings, rooted, host): (&[usize], &[QualityBinding<'_>], &RuleContext<'_>, Host),
  (state, trace): (&mut WalkState, &mut Vec<Step>),
) -> Option<ModuleResult> {
  let mut batch = QualityBatch::new(&selected.records);
  for (record, edit) in selected.records.iter().enumerate() {
    let extension = Path::new(&edit.path)
      .extension()
      .and_then(|name| name.to_str());
    for &index in active {
      let Some(binding) = bindings.get(index) else {
        continue;
      };
      if !extension.is_some_and(|name| binding.rule.extensions().contains(&name)) {
        continue;
      }
      let (decision, warnings) = batch.decide(record, (index, active, bindings), rooted);
      selected.warnings.extend(warnings);
      trace.push(Step {
        module: format!("{}__{}.json", binding.spec, binding.name),
        kind: StepKind::Rule,
        status: StepStatus::Decided,
      });
      let folded = Folded {
        name: binding.name.to_owned(),
        result: ModuleResult {
          stdout: encoded(host, HostEvent::ToolPost, &decision),
          ..ModuleResult::default()
        },
        truncated: false,
      };
      if let Some(done) = state.consume(&folded) {
        return Some(done);
      }
    }
  }
  None
}

fn shell_quality(
  (payload, ctx, session): (&Payload<'_>, &RuleContext<'_>, &Session<'_>),
  plan: &RegistryPlan<'_>,
  (state, trace): (&mut WalkState, &mut Vec<Step>),
) -> Option<ModuleResult> {
  if session.phase != Phase::Post || !matches!(payload.tool_name, "Bash" | "Shell") {
    return None;
  }
  let active = plan.active_quality();
  let bindings = session.options.quality_rules;
  let mut selected = shell::collect(ctx, active, bindings)?;
  let rooted = RuleContext {
    cwd: Some(&session.project_root),
    ..*ctx
  };
  let mut stopped = shell_decisions(
    &mut selected,
    (active, bindings, &rooted, session.host),
    (state, trace),
  );
  if let Some(advisory) = selected.advisory
    && let Ok(message) = toolu_protocol::text::Text::new(advisory)
  {
    let folded = Folded {
      name: "quality-shell".to_owned(),
      result: ModuleResult {
        stdout: encoded(
          session.host,
          HostEvent::ToolPost,
          &Decision::Advisory { message },
        ),
        ..ModuleResult::default()
      },
      truncated: false,
    };
    stopped = state.consume(&folded).or(stopped);
  }
  shell::touch_marker(&selected.gate, &mut selected.warnings);
  for warning in selected.warnings {
    state.stderr.push_str(&warning);
    state.stderr.push('\n');
  }
  stopped
}

/// Walk `payload`: built-ins, then the registry.
pub(crate) fn walk(
  payload: &Payload<'_>,
  session: &Session<'_>,
  trace: &mut Vec<Step>,
) -> ModuleResult {
  let view = match View::of(payload, session) {
    Ok(view) => view,
    Err(message) => return failed(session.phase, &message),
  };
  walk_with_view(payload, &view, session, trace, (&mut None, None))
}

/// Walk a split path against the outer event's reused view.
pub(crate) fn walk_with_view<'a>(
  payload: &Payload<'_>,
  view: &View,
  session: &Session<'a>,
  trace: &mut Vec<Step>,
  (registry_plan, quality): (
    &mut Option<RegistryPlan<'a>>,
    Option<(&RefCell<QualityBatch>, usize)>,
  ),
) -> ModuleResult {
  // One memo per path walk. The next event or edit path must not see it.
  let _event_memo = crate::gates::EventMemoGuard::enter();
  let ctx = view.rule_context(payload, session);
  let event = host_event(&view.event);
  let mut state = WalkState::new(session.phase, session.host);
  for gate in session.options.builtins {
    let folded = run_gate(
      *gate,
      (view, &ctx),
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
    view,
    ctx: &ctx,
    event,
    quality,
  };
  let plan = registry_plan.get_or_insert_with(|| RegistryPlan::new(session, payload.tool_name));
  for folded in run_registry(&registry, &mut state.stderr, trace, plan) {
    if let Some(done) = state.consume(&folded) {
      return done;
    }
  }
  if let Some(done) = shell_quality((payload, &ctx, session), plan, (&mut state, trace)) {
    return done;
  }
  state.settle()
}

#[cfg(test)]
#[path = "tests/walk_test.rs"]
mod tests;
