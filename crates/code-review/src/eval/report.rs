//! The paired-comparison report: per arm, the findings, coverage, every model call
//! with its `live` or `replayed` origin, usage, cost, runtime, tokens-per-finding and
//! the expectation matches. A replayed call carries its tokens but charges nothing,
//! so a replayed baseline adds no second bill. Arms are judged on expectation
//! outcomes, never on fewer findings or fewer tokens alone.

use serde::Serialize;

use super::matcher::{Match, Outcome, Tally};
use crate::budget::Effort;
use crate::contract::{Coverage, Finding};
use crate::review::{Call, Stage};

/// The report document's version.
pub const SCHEMA: u32 = 1;

/// Which side of the paired comparison an arm is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Arm {
  /// The review without the Jev judge.
  Baseline,
  /// The review with the Jev judge.
  Enhanced,
}

/// Where a call's response came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LiveOrReplayed {
  /// A real network call, billed.
  Live,
  /// A recorded response; nothing was sent or charged.
  Replayed,
}

/// One model call in the report: the usage log's `Call` plus its origin.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CallRecord {
  /// The stage that made it.
  pub stage: Stage,
  /// The model requested or served.
  pub model: String,
  /// The serving provider, when reported.
  pub provider: Option<String>,
  /// Serving quantization, when reported.
  pub quantization: Option<String>,
  /// Whether it was sent or replayed.
  pub live_or_replayed: LiveOrReplayed,
  /// Input plus output tokens.
  pub tokens: u64,
  /// What it billed: zero for a replayed call, unknown when a live call reported none.
  pub cost: Option<f64>,
  /// Wall time in milliseconds.
  pub millis: u64,
  #[serde(skip)]
  tokens_in: u64,
  #[serde(skip)]
  tokens_out: u64,
}

impl CallRecord {
  /// `call` as the report shows it, with `origin`.
  pub fn from_call(call: &Call, origin: LiveOrReplayed) -> CallRecord {
    CallRecord {
      stage: call.stage,
      model: call.model.clone(),
      provider: call.provider.clone(),
      quantization: call.quantization.clone(),
      live_or_replayed: origin,
      tokens: call.tokens_in.saturating_add(call.tokens_out),
      cost: match origin {
        LiveOrReplayed::Live => call.cost_usd,
        LiveOrReplayed::Replayed => Some(0.0),
      },
      millis: call.millis,
      tokens_in: call.tokens_in,
      tokens_out: call.tokens_out,
    }
  }
}

/// Token and cost totals of an arm.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
pub struct UsageTotals {
  /// Prompt tokens over every call, live or replayed.
  pub tokens_in: u64,
  /// Completion tokens over every call, live or replayed.
  pub tokens_out: u64,
  /// Billed dollars: live calls only; unreported costs count as zero.
  pub cost_usd: f64,
}

/// How many findings an arm reported and how many of them block.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct FindingsSummary {
  /// Every finding.
  pub total: usize,
  /// Findings at `medium` or above.
  pub blocking: usize,
}

impl FindingsSummary {
  /// Count `findings`.
  pub fn of(findings: &[Finding]) -> FindingsSummary {
    FindingsSummary {
      total: findings.len(),
      blocking: findings
        .iter()
        .filter(|finding| finding.severity.blocking())
        .count(),
    }
  }
}

/// One arm of the comparison at one effort level.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ArmReport {
  /// Baseline or enhanced.
  pub arm: Arm,
  /// The effort level.
  pub effort: Effort,
  /// What the arm reported.
  pub findings: FindingsSummary,
  /// What it covered.
  pub coverage: Coverage,
  /// Every model call.
  pub calls: Vec<CallRecord>,
  /// Token and cost totals.
  pub usage: UsageTotals,
  /// Wall time of the run in milliseconds.
  pub runtime_ms: u64,
  /// Total tokens per finding; `None` when the arm reported no finding.
  pub tokens_per_finding: Option<f64>,
  /// Each expectation's outcome.
  pub matches: Vec<Match>,
  /// Status counts over `matches` and `unclassified`.
  pub tally: Tally,
  /// Findings no expectation claimed, for human adjudication.
  pub unclassified: Vec<Finding>,
}

/// Convert exactly enough for a ratio: the two 32-bit halves are exact in `f64`.
fn as_f64(value: u64) -> f64 {
  let high = u32::try_from(value >> 32).unwrap_or(u32::MAX);
  let low = u32::try_from(value & u64::from(u32::MAX)).unwrap_or(u32::MAX);
  f64::from(high) * 4_294_967_296.0 + f64::from(low)
}

/// What one arm's run produced, before the matcher's verdict.
#[derive(Debug, Clone, PartialEq)]
pub struct Run {
  /// Baseline or enhanced.
  pub arm: Arm,
  /// The effort level.
  pub effort: Effort,
  /// What the run covered.
  pub coverage: Coverage,
  /// The run's calls with their origin.
  pub calls: Vec<CallRecord>,
  /// Wall time of the run in milliseconds.
  pub runtime_ms: u64,
}

impl ArmReport {
  /// Shape one arm: `run` is what it did, `findings` what it reported, and `outcome`
  /// the matcher's verdict over them.
  pub fn new(run: Run, findings: &[Finding], outcome: Outcome) -> ArmReport {
    let calls = run.calls;
    let usage = UsageTotals {
      tokens_in: calls.iter().map(|call| call.tokens_in).sum(),
      tokens_out: calls.iter().map(|call| call.tokens_out).sum(),
      cost_usd: calls.iter().filter_map(|call| call.cost).sum(),
    };
    let summary = FindingsSummary::of(findings);
    let count = u64::try_from(summary.total).unwrap_or(u64::MAX);
    let tokens_per_finding =
      (count > 0).then(|| as_f64(usage.tokens_in.saturating_add(usage.tokens_out)) / as_f64(count));
    ArmReport {
      arm: run.arm,
      effort: run.effort,
      findings: summary,
      coverage: run.coverage,
      calls,
      usage,
      runtime_ms: run.runtime_ms,
      tokens_per_finding,
      tally: outcome.tally(),
      matches: outcome.matches,
      unclassified: outcome.unclassified,
    }
  }
}

/// The two revisions the diff spans.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Revisions {
  /// The base of the diff.
  pub base: String,
  /// The tip of the diff.
  pub tip: String,
}

/// The whole eval document.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EvalReport {
  /// Always [`SCHEMA`].
  pub schema: u32,
  /// The pinned diff.
  pub revisions: Revisions,
  /// Every arm that ran.
  pub arms: Vec<ArmReport>,
}

impl EvalReport {
  /// A report over `arms` of the diff `revisions`.
  pub fn new(revisions: Revisions, arms: Vec<ArmReport>) -> EvalReport {
    EvalReport {
      schema: SCHEMA,
      revisions,
      arms,
    }
  }
}

#[cfg(test)]
#[path = "tests/report_test.rs"]
mod tests;
