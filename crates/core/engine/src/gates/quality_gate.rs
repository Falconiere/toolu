//! Pre-tool quality gate: a recorded failure blocks commit and push.

use toolu_protocol::decision::Decision;
use toolu_protocol::normalized::NormalizedEvent;
use toolu_runtime::config::gate_mode::GateMode;
use toolu_runtime::git::toplevel;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::json::{jq_text, ordered::Ordered};
use toolu_runtime::registry::rule::RuleContext;
use toolu_shell::analysis::ShellAnalysis;
use toolu_state::gate_file::{GateRead, read_gate_file};
use toolu_state::git::{has_git, linked_worktree};

use super::{command_analysis, decided, gate_config, pre_mode};
use crate::detect::{is_git_commit, is_git_push};
use crate::gate::Gate;

/// The pre-tool quality-state gate.
pub(crate) struct QualityGate;
/// Its singleton in the ordered pre-tool table.
pub(crate) static QUALITY_GATE: QualityGate = QualityGate;

fn may_ship(analysis: &ShellAnalysis) -> bool {
  analysis.unknown || is_git_commit(analysis) || is_git_push(analysis)
}

fn field(doc: &Ordered, key: &str, fallback: &str) -> String {
  let Some(value) = doc.get(key) else {
    return fallback.to_owned();
  };
  match value {
    Ordered::Null | Ordered::Bool(false) => fallback.to_owned(),
    Ordered::String(text) => text.trim_end_matches('\n').to_owned(),
    Ordered::Bool(_) | Ordered::Number(_) | Ordered::Array(_) | Ordered::Object(_) => {
      jq_text(value, true).trim_end_matches('\n').to_owned()
    }
  }
}

fn failing_doc(read: GateRead) -> Option<Ordered> {
  let doc = match read {
    GateRead::Ok(doc) => doc.to_ordered(),
    GateRead::Unrecognized { value, .. } => value,
    GateRead::Missing | GateRead::Malformed(_) => return None,
  };
  matches!(doc, Ordered::Object(_))
    .then(|| doc)
    .filter(|doc| field(doc, "status", "") == "failing")
}

fn lead(mode: GateMode) -> &'static str {
  match mode {
    GateMode::Block => {
      "BLOCKED: quality gate failing — fix the violations before committing or pushing."
    }
    GateMode::Ask => "The quality gate is failing. Commit/push anyway?",
    GateMode::Advise => {
      "Heads up: the quality gate is failing (commit and push are not blocked at this setting)."
    }
    GateMode::Off => "",
  }
}

impl Gate for QualityGate {
  fn name(&self) -> &'static str {
    "quality-gate"
  }

  fn run(&self, event: &NormalizedEvent, ctx: &RuleContext<'_>) -> Result<Decision, String> {
    if !matches!(event, NormalizedEvent::ShellPre { .. })
      || ctx.env.get("MY_CLAUDE_QUALITY") == Some("off")
    {
      return Ok(Decision::Allow);
    }
    if !may_ship(&command_analysis(ctx)) || !has_git(ctx.env) {
      return Ok(Decision::Allow);
    }
    let mode = pre_mode(&gate_config(ctx), "qualityGate", ctx, true);
    if mode == GateMode::Off {
      return Ok(Decision::Allow);
    }
    let cwd = ctx.cwd.unwrap_or(ctx.project_root);
    let root = toplevel(ctx.env, cwd).unwrap_or_else(|| cwd.to_path_buf());
    let roots = Roots::new(ctx.env.clone(), Some(ctx.host));
    let Some(state_root) = roots.project_state_root(None, Some(&root)) else {
      return Ok(Decision::Allow);
    };
    let gate = state_root.join("quality-gate-status.json");
    let Some(doc) = failing_doc(read_gate_file(&gate)) else {
      return Ok(Decision::Allow);
    };
    if linked_worktree(ctx.env, &root) {
      return Ok(Decision::Allow);
    }
    let reason = field(&doc, "reason", "Quality gate failing");
    let violations = field(&doc, "violations", "");
    decided(mode, format!("{}\n{reason}\n{violations}", lead(mode)))
  }
}

#[cfg(test)]
#[path = "tests/quality_gate_test.rs"]
mod tests;
