//! PostToolUse gate-status (`gate-status.ts`, the native port of
//! `post-tools/modules/gate-status.sh`): after a Bash/Shell call that ran a
//! quality command, record the command channel of the quality gate
//! (`__global__`, source `gate-status-hook`) as failing, or clear it.
//!
//! A quality command must be one the line runs, and a zero exit of the line
//! must prove it passed (`exit_proves`: not piped, not behind `||`, not followed
//! by `;`). A pass is recorded only when every quality command is proven, so
//! `bun test | tail; bun run lint` exiting 0 never vouches for the tests; a
//! failure when at least one is. Anything else leaves the slot as it was.

use std::path::{Path, PathBuf};

use toolu_protocol::decision::Decision;
use toolu_protocol::normalized::NormalizedEvent;
use toolu_protocol::text::Text;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::json::jq_text;
use toolu_runtime::json::ordered::Ordered;
use toolu_runtime::registry::rule::RuleContext;
use toolu_state::ctx::StateCtx;
use toolu_state::gate_file::{GateFailure, clear_gate_file, record_gate_failure};
use toolu_state::gate_schema::GLOBAL_GATE_KEY;
use toolu_state::time::iso_seconds;

use super::quality_command::{QualityCommand, quality_commands};
use super::tool_exit::{tool_command, tool_exit_status};
use super::{command_analysis, is_shell_tool};
use crate::gate::Gate;

/// The gate's slot source.
const SOURCE: &str = "gate-status-hook";

/// The built-in `gate-status.sh`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct GateStatus;

/// The one `gate-status.sh` built-in.
pub(crate) static GATE_STATUS: GateStatus = GateStatus;

/// The advisory bash prints. Its `\n` is two characters: bash passes the
/// double-quoted `\n` to `jq --arg` unexpanded.
fn failing_context(command: &str, status: &str) -> String {
  format!(
    "Global quality gate failing. Fix all errors/warnings/tests before new tasks.\\nFailed: {command} (exit {status})"
  )
}

/// `$GATE_DIR`: `<PROJECT_ROOT>/<.host>/tmp`, created as `mkdir -p` does.
fn gate_dir(roots: &Roots, ctx: &RuleContext<'_>) -> Result<PathBuf, String> {
  let dir = roots
    .project_state_root(None, Some(ctx.project_root))
    .ok_or_else(|| "gate-status: no project state root".to_owned())?;
  std::fs::create_dir_all(&dir).map_err(|err| format!("gate-status: {}: {err}", dir.display()))?;
  Ok(dir)
}

/// The first-ever pass, with nothing tracked yet: `jq -n ... > "$GATE_FILE"`,
/// pretty, its source the command. A plain write, as bash's redirect is.
fn write_first_pass(state: &StateCtx, gate: &Path, command: &str) -> Result<(), String> {
  let doc = Ordered::Object(vec![
    ("status".to_owned(), Ordered::String("passing".to_owned())),
    ("source".to_owned(), Ordered::String(command.to_owned())),
    (
      "updatedAt".to_owned(),
      Ordered::String(iso_seconds(state.now())),
    ),
  ]);
  std::fs::write(gate, format!("{}\n", jq_text(&doc, true)))
    .map_err(|err| format!("gate-status: {}: {err}", gate.display()))
}

/// Whether `status` is all digits and not a zero (`/^[0-9]+$/` and `!== 0`).
fn failed(status: &str) -> bool {
  !status.is_empty()
    && status.bytes().all(|b| b.is_ascii_digit())
    && status.bytes().any(|b| b != b'0')
}

impl Gate for GateStatus {
  fn name(&self) -> &'static str {
    "gate-status.sh"
  }

  fn run(&self, event: &NormalizedEvent, ctx: &RuleContext<'_>) -> Result<Decision, String> {
    self.run_warning(event, ctx, &mut Vec::new())
  }

  fn run_warning(
    &self,
    event: &NormalizedEvent,
    ctx: &RuleContext<'_>,
    warnings: &mut Vec<String>,
  ) -> Result<Decision, String> {
    if !is_shell_tool(event) {
      return Ok(Decision::Allow);
    }
    let roots = Roots::new(ctx.env.clone(), Some(ctx.host));
    let gate = gate_dir(&roots, ctx)?.join("quality-gate-status.json");
    let analysis = command_analysis(ctx);
    let quality = quality_commands(&analysis);
    if quality.is_empty() {
      return Ok(Decision::Allow);
    }
    let mut state = StateCtx::new(roots);
    let decided = record(&mut state, &gate, &quality, ctx);
    warnings.append(&mut state.warnings);
    decided
  }
}

/// Records what the line's exit status proves about `quality` in `gate`.
fn record(
  state: &mut StateCtx,
  gate: &Path,
  quality: &[QualityCommand<'_>],
  ctx: &RuleContext<'_>,
) -> Result<Decision, String> {
  let command = tool_command(ctx.raw);
  let status = tool_exit_status(ctx.raw);
  if failed(&status) {
    if !quality.iter().any(|q| q.command.exit_proves) {
      return Ok(Decision::Allow);
    }
    let reason = format!("Quality command failed: {command} (exit {status})");
    let failure = GateFailure {
      file: GLOBAL_GATE_KEY,
      source: SOURCE,
      reason: &reason,
      violations: "",
    };
    record_gate_failure(state, gate, &failure);
    let message = Text::new(failing_context(&command, &status)).map_err(|err| err.to_string())?;
    return Ok(Decision::Advisory { message });
  }
  if status == "0" && quality.iter().all(|q| q.command.exit_proves) {
    clear_gate_file(state, gate, GLOBAL_GATE_KEY, SOURCE);
    if !gate.exists() {
      write_first_pass(state, gate, &command)?;
    }
  }
  Ok(Decision::Allow)
}

#[cfg(test)]
#[path = "tests/gate_status_test.rs"]
mod tests;
