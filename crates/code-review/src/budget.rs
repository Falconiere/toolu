//! Every byte and token cap of a review run, in one place so each stage enforces
//! the same numbers (epic #660 budgets). The caps keep prompts inside the context
//! window and bound what the agent reads; they are not spend limits. The starting
//! values come from the upstream action and `.github/workflows/toolu-review.yml`,
//! and #670 calibrates them.

use serde::{Deserialize, Serialize};

const KIB: usize = 1024;

/// Repository conventions sent with a review (`RULES_MAX_BYTES` in the CI review).
pub const RULES_MAX_BYTES: usize = 64 * KIB;
/// The review checklist (about 4.4 KiB upstream).
pub const CHECKLIST_MAX_BYTES: usize = 4_506;
/// Jev state plus its question, per judgment.
pub const JEV_STATE_MAX_BYTES: usize = 90_000;
/// One Jev request body.
pub const JEV_REQUEST_MAX_BYTES: usize = 180_000;
/// One model response body; explicit rather than the `core/http` default.
pub const HTTP_RESPONSE_MAX_BYTES: usize = 1024 * KIB;

/// How much a run reviews around the diff. Every level reviews every changed line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Effort {
  /// Jev picks the level per run (#666).
  Auto,
  /// Small, contained changes.
  Small,
  /// The default when Jev cannot pick.
  Medium,
  /// Several modules, new interfaces, persistence or concurrency.
  High,
  /// Security, workflows, or cross-cutting changes.
  Xhigh,
}

/// How deeply Jev judges the model's findings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Judge {
  /// Blocking findings only, no recheck.
  BlockingOnly,
  /// Every fresh finding, plus a recheck of prior ones.
  AllWithRecheck,
  /// As `AllWithRecheck`, with related cross-file context.
  AllWithContext,
}

/// The caps of one effort level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
  /// Surrounding source per changed file.
  pub source_file_bytes: usize,
  /// Surrounding source per package.
  pub source_package_bytes: usize,
  /// Jev judge depth.
  pub judge: Judge,
  /// The model's `reasoning.effort`.
  pub reasoning: &'static str,
  /// `max_tokens`: reasoning plus output.
  pub max_tokens: u32,
  /// The review call's wall-clock deadline.
  pub deadline_secs: u64,
}

const SMALL: Budget = Budget {
  source_file_bytes: 8 * KIB,
  source_package_bytes: 16 * KIB,
  judge: Judge::BlockingOnly,
  reasoning: "low",
  max_tokens: 16_384,
  deadline_secs: 180,
};

const MEDIUM: Budget = Budget {
  source_file_bytes: 16 * KIB,
  source_package_bytes: 32 * KIB,
  judge: Judge::AllWithRecheck,
  reasoning: "medium",
  max_tokens: 32_768,
  deadline_secs: 360,
};

const HIGH: Budget = Budget {
  source_file_bytes: 32 * KIB,
  source_package_bytes: 64 * KIB,
  judge: Judge::AllWithRecheck,
  reasoning: "high",
  max_tokens: 65_536,
  deadline_secs: 720,
};

const XHIGH: Budget = Budget {
  source_file_bytes: 64 * KIB,
  source_package_bytes: 128 * KIB,
  judge: Judge::AllWithContext,
  reasoning: "xhigh",
  max_tokens: 131_072,
  deadline_secs: 1200,
};

impl Effort {
  /// Every level, in `--effort` order.
  pub const ALL: [Effort; 5] = [
    Effort::Auto,
    Effort::Small,
    Effort::Medium,
    Effort::High,
    Effort::Xhigh,
  ];

  /// The level a run uses when Jev cannot pick under `auto`.
  pub const FALLBACK: Effort = Effort::Medium;

  /// The name `--effort`, the config and the report use.
  pub fn name(self) -> &'static str {
    match self {
      Effort::Auto => "auto",
      Effort::Small => "small",
      Effort::Medium => "medium",
      Effort::High => "high",
      Effort::Xhigh => "xhigh",
    }
  }

  /// The level named `name`.
  pub fn parse(name: &str) -> Option<Effort> {
    Effort::ALL.into_iter().find(|level| level.name() == name)
  }

  /// The caps of this level; `auto` budgets as the fallback until Jev picks.
  pub fn budget(self) -> &'static Budget {
    match self {
      Effort::Small => &SMALL,
      Effort::Auto | Effort::Medium => &MEDIUM,
      Effort::High => &HIGH,
      Effort::Xhigh => &XHIGH,
    }
  }
}

#[cfg(test)]
#[path = "tests/budget_test.rs"]
mod tests;
