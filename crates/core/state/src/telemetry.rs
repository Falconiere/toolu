//! Workflow telemetry (`telemetry.ts`): one compact line
//! `{...extras, "v":1, "t", "branch", "event"}` appended to
//! `<state root>/telemetry/<branch slug>.jsonl` (`$TELEMETRY_DIR` replaces the
//! directory). The events are a closed enum carrying exactly their extras, so
//! a command line or a tool payload cannot reach the log. Every opt-out and
//! rejected line is a [`TelemetryResult::Skipped`], never an error.

use std::io::Write as _;
use std::path::{Path, PathBuf};

use serde_json::Number;
use toolu_runtime::config::read::enabled;
use toolu_runtime::json::jq_text;
use toolu_runtime::json::ordered::Ordered;

use crate::ctx::StateCtx;
use crate::git::{branch_slug, current_branch};
use crate::time::iso_seconds;

/// The `v` every line carries.
pub const TELEMETRY_VERSION: u8 = 1;
/// Every event name, in `TELEMETRY_EXTRAS` order.
pub const TELEMETRY_EVENTS: [&str; 8] = [
  "gate_fail",
  "gate_clear",
  "step_run",
  "ac_coverage",
  "docs_attested",
  "docs_nudge",
  "push_check",
  "delegation",
];
/// Headroom under the 4 KiB atomic-append floor: one append stays whole.
pub const TELEMETRY_MAX_LINE_BYTES: usize = 3900;

/// Every event a caller records, with its exact extras (`TELEMETRY_EXTRAS`).
#[derive(Debug, Clone, PartialEq)]
pub enum TelemetryEvent {
  /// A gate failure was recorded.
  GateFail {
    /// The failing file.
    file: String,
    /// The gate that failed it.
    source: String,
  },
  /// A gate entry was cleared.
  GateClear {
    /// The cleared file.
    file: String,
    /// The gate that cleared it.
    source: String,
  },
  /// A plan step ran.
  StepRun {
    /// The step id.
    step_id: String,
    /// Its status.
    status: String,
    /// Its check's exit code.
    exit_code: f64,
    /// How long it ran.
    duration_s: f64,
    /// Which attempt this was.
    attempt: f64,
  },
  /// Acceptance-criteria coverage.
  AcCoverage {
    /// Covered criteria.
    covered: f64,
    /// Uncovered criteria.
    uncovered: f64,
  },
  /// A docs decision was attested.
  DocsAttested {
    /// The decision.
    decision: String,
  },
  /// A docs nudge was shown.
  DocsNudge,
  /// A push was checked.
  PushCheck {
    /// The result.
    result: String,
    /// Why.
    reason_code: String,
    /// The review round, if any.
    round: Option<f64>,
  },
  /// A delegation was recorded.
  Delegation {
    /// The model.
    model: Option<String>,
    /// The subagent type.
    subagent_type: Option<String>,
    /// The reasoning effort.
    reasoning_effort: Option<String>,
    /// The plan step.
    step_id: Option<String>,
    /// The plan step's model.
    step_model: Option<String>,
  },
}

impl TelemetryEvent {
  /// The event's name in the log.
  pub fn name(&self) -> &'static str {
    match self {
      TelemetryEvent::GateFail { .. } => "gate_fail",
      TelemetryEvent::GateClear { .. } => "gate_clear",
      TelemetryEvent::StepRun { .. } => "step_run",
      TelemetryEvent::AcCoverage { .. } => "ac_coverage",
      TelemetryEvent::DocsAttested { .. } => "docs_attested",
      TelemetryEvent::DocsNudge => "docs_nudge",
      TelemetryEvent::PushCheck { .. } => "push_check",
      TelemetryEvent::Delegation { .. } => "delegation",
    }
  }

  /// The extras in schema order; `None` when a number is not finite.
  fn extras(&self) -> Option<Vec<(String, Ordered)>> {
    let fields = match self {
      TelemetryEvent::GateFail { file, source } | TelemetryEvent::GateClear { file, source } => {
        vec![
          field("file", Value::Text(file)),
          field("source", Value::Text(source)),
        ]
      }
      TelemetryEvent::AcCoverage { covered, uncovered } => {
        vec![
          field("covered", Value::Number(*covered)),
          field("uncovered", Value::Number(*uncovered)),
        ]
      }
      TelemetryEvent::DocsAttested { decision } => vec![field("decision", Value::Text(decision))],
      TelemetryEvent::DocsNudge => Vec::new(),
      TelemetryEvent::StepRun { .. }
      | TelemetryEvent::PushCheck { .. }
      | TelemetryEvent::Delegation { .. } => self.detailed_extras(),
    };
    fields.into_iter().collect()
  }

  /// The extras of the events with three or more fields.
  fn detailed_extras(&self) -> Vec<Option<(String, Ordered)>> {
    match self {
      TelemetryEvent::StepRun {
        step_id,
        status,
        exit_code,
        duration_s,
        attempt,
      } => vec![
        field("step_id", Value::Text(step_id)),
        field("status", Value::Text(status)),
        field("exit_code", Value::Number(*exit_code)),
        field("duration_s", Value::Number(*duration_s)),
        field("attempt", Value::Number(*attempt)),
      ],
      TelemetryEvent::PushCheck {
        result,
        reason_code,
        round,
      } => vec![
        field("result", Value::Text(result)),
        field("reason_code", Value::Text(reason_code)),
        field("round", Value::MaybeNumber(*round)),
      ],
      TelemetryEvent::Delegation {
        model,
        subagent_type,
        reasoning_effort,
        step_id,
        step_model,
      } => [
        ("model", model),
        ("subagent_type", subagent_type),
        ("reasoning_effort", reasoning_effort),
        ("step_id", step_id),
        ("step_model", step_model),
      ]
      .into_iter()
      .map(|(key, value)| field(key, Value::MaybeText(value.as_deref())))
      .collect(),
      TelemetryEvent::GateFail { .. }
      | TelemetryEvent::GateClear { .. }
      | TelemetryEvent::AcCoverage { .. }
      | TelemetryEvent::DocsAttested { .. }
      | TelemetryEvent::DocsNudge => Vec::new(),
    }
  }
}

/// One extra's value, as its schema types it.
#[derive(Clone, Copy)]
enum Value<'a> {
  Text(&'a str),
  MaybeText(Option<&'a str>),
  Number(f64),
  MaybeNumber(Option<f64>),
}

/// `key` with `value`; `None` for a number that is not finite (zod's `z.number()`).
fn field(key: &str, value: Value<'_>) -> Option<(String, Ordered)> {
  let value = match value {
    Value::Text(text) | Value::MaybeText(Some(text)) => Ordered::String(text.to_owned()),
    Value::MaybeText(None) | Value::MaybeNumber(None) => Ordered::Null,
    Value::Number(number) | Value::MaybeNumber(Some(number)) => {
      Ordered::Number(Number::from_f64(number)?)
    }
  };
  Some((key.to_owned(), value))
}

/// What an append did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TelemetryResult {
  /// The line was appended to this file.
  Written(PathBuf),
  /// Nothing was written, and why.
  Skipped(String),
}

/// The line for `event` on `branch` at `t`, or why it cannot be written.
pub(crate) fn assemble(event: &TelemetryEvent, branch: &str, t: &str) -> Result<String, String> {
  let name = event.name();
  let Some(mut fields) = event.extras() else {
    return Err(format!(
      "telemetry: invalid extras for event \"{name}\"; skipping append"
    ));
  };
  fields.extend([
    ("v".to_owned(), Ordered::Number(TELEMETRY_VERSION.into())),
    ("t".to_owned(), Ordered::String(t.to_owned())),
    ("branch".to_owned(), Ordered::String(branch.to_owned())),
    ("event".to_owned(), Ordered::String(name.to_owned())),
  ]);
  let line = jq_text(&Ordered::Object(fields), false);
  if line.len() > TELEMETRY_MAX_LINE_BYTES {
    return Err(format!(
      "telemetry: assembled line for event \"{name}\" is {} bytes (>{TELEMETRY_MAX_LINE_BYTES}); skipping append",
      line.len()
    ));
  }
  Ok(line)
}

/// `telemetry_append ROOT EVENT`: appends `event` for the branch of `root`.
pub fn telemetry_append(
  ctx: &mut StateCtx,
  root: &Path,
  event: &TelemetryEvent,
) -> TelemetryResult {
  let skip = |reason: &str| TelemetryResult::Skipped(reason.to_owned());
  if root.as_os_str().is_empty() {
    return skip("no root");
  }
  // Default-on: only an explicit false turns telemetry off.
  if !enabled(ctx.config(root), "telemetry", "enabled") {
    return skip("disabled");
  }
  let branch = current_branch(ctx.roots.env(), root);
  // An unborn or detached HEAD would collapse every caller onto `_default`.
  if branch.is_empty() || branch == "HEAD" {
    return skip("no branch");
  }
  let line = match assemble(event, &branch, &iso_seconds(ctx.now())) {
    Ok(line) => line,
    Err(reason) => {
      ctx.warnings.push(reason.clone());
      return TelemetryResult::Skipped(reason);
    }
  };
  let dir = match ctx.roots.env().get("TELEMETRY_DIR") {
    Some(dir) => Some(PathBuf::from(dir)),
    None => ctx
      .roots
      .project_state_dir("telemetry", None, Some(root))
      .ok()
      .flatten(),
  };
  let Some(dir) = dir else {
    return skip("no state dir");
  };
  let file = dir.join(format!("{}.jsonl", branch_slug(&branch)));
  match append(&dir, &file, &line) {
    Ok(()) => TelemetryResult::Written(file),
    Err(err) => TelemetryResult::Skipped(format!("could not append to {}: {err}", file.display())),
  }
}

fn append(dir: &Path, file: &Path, line: &str) -> std::io::Result<()> {
  std::fs::create_dir_all(dir)?;
  let mut out = std::fs::OpenOptions::new()
    .create(true)
    .append(true)
    .open(file)?;
  out.write_all(format!("{line}\n").as_bytes())
}

#[cfg(test)]
#[path = "tests/telemetry_test.rs"]
mod tests;
