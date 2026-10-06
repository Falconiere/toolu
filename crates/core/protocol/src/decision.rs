//! `Decision`: what a gate decided (`packages/toolu-core/src/decision/decision.ts`),
//! and how several decisions merge (`mergeDecisions` in `policy/policy.ts`).
//!
//! The wire is TypeScript's: `{"kind":"deny","reason":…}`. Unlike zod's
//! `z.object`, which strips unknown keys, it is strict: an unknown field or a
//! field of another kind fails to parse.

use serde::{Deserialize, Serialize};

use crate::text::Text;

/// One gate's decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Wire", into = "Wire")]
pub enum Decision {
  /// Proceed silently.
  Allow,
  /// Ask the human first, where the host can prompt.
  Ask {
    /// Why.
    reason: Text,
  },
  /// Refuse the action.
  Deny {
    /// Why.
    reason: Text,
  },
  /// Proceed, with context for the model.
  Advisory {
    /// The context.
    message: Text,
  },
  /// After a tool ran: report the result as blocked (`post_block`).
  Block {
    /// Why.
    reason: Text,
  },
  /// The gate itself failed; encoders fail closed on blocking events.
  RuntimeFailure {
    /// What failed.
    reason: Text,
    /// How it failed.
    code: FailureCode,
  },
}

/// How a gate failed to decide.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureCode {
  /// It ran past its deadline.
  Timeout,
  /// It could not be started.
  Spawn,
  /// Its output did not parse.
  Parse,
  /// Its output was cut off.
  Truncated,
  /// It was cancelled.
  Cancelled,
  /// It exited non-zero.
  Nonzero,
}

/// Which kind of gate an `ask` came from, for hosts that cannot prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateClass {
  /// A security guardrail: its ask becomes a deny.
  Guardrail,
  /// A judgement gate: its ask becomes advice.
  Judgement,
}

/// The decision that wins: the first of the highest rank, where runtime failure
/// ranks above deny and block, then ask, advisory and allow. An empty list is allow.
pub fn merge(decisions: &[Decision]) -> Decision {
  let mut best: Option<&Decision> = None;
  for decision in decisions {
    if best.is_none_or(|current| rank(decision) > rank(current)) {
      best = Some(decision);
    }
  }
  best.cloned().unwrap_or(Decision::Allow)
}

/// `MERGE_RANK` in `policy.ts`.
fn rank(decision: &Decision) -> u8 {
  match decision {
    Decision::RuntimeFailure { .. } => 6,
    Decision::Deny { .. } | Decision::Block { .. } => 5,
    Decision::Ask { .. } => 4,
    Decision::Advisory { .. } => 3,
    Decision::Allow => 1,
  }
}

/// The flat wire every kind shares; [`Decision`] checks which fields each kind takes.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
  kind: Kind,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  reason: Option<Text>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  message: Option<Text>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  code: Option<FailureCode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Kind {
  Allow,
  Ask,
  Deny,
  Advisory,
  PostBlock,
  RuntimeFailure,
}

impl TryFrom<Wire> for Decision {
  type Error = String;

  fn try_from(wire: Wire) -> Result<Decision, String> {
    let Wire {
      kind,
      reason,
      message,
      code,
    } = wire;
    let decision = match (kind, reason, message, code) {
      (Kind::Allow, None, None, None) => Decision::Allow,
      (Kind::Ask, Some(reason), None, None) => Decision::Ask { reason },
      (Kind::Deny, Some(reason), None, None) => Decision::Deny { reason },
      (Kind::Advisory, None, Some(message), None) => Decision::Advisory { message },
      (Kind::PostBlock, Some(reason), None, None) => Decision::Block { reason },
      (Kind::RuntimeFailure, Some(reason), None, Some(code)) => {
        Decision::RuntimeFailure { reason, code }
      }
      (kind, ..) => return Err(wrong_fields(kind)),
    };
    Ok(decision)
  }
}

fn wrong_fields(kind: Kind) -> String {
  match kind {
    Kind::Allow => "an `allow` decision takes no fields".to_owned(),
    Kind::Ask => "an `ask` decision takes exactly `reason`".to_owned(),
    Kind::Deny => "a `deny` decision takes exactly `reason`".to_owned(),
    Kind::Advisory => "an `advisory` decision takes exactly `message`".to_owned(),
    Kind::PostBlock => "a `post_block` decision takes exactly `reason`".to_owned(),
    Kind::RuntimeFailure => {
      "a `runtime_failure` decision takes exactly `reason` and `code`".to_owned()
    }
  }
}

impl From<Decision> for Wire {
  fn from(decision: Decision) -> Wire {
    let (kind, reason, message, code) = match decision {
      Decision::Allow => (Kind::Allow, None, None, None),
      Decision::Ask { reason } => (Kind::Ask, Some(reason), None, None),
      Decision::Deny { reason } => (Kind::Deny, Some(reason), None, None),
      Decision::Advisory { message } => (Kind::Advisory, None, Some(message), None),
      Decision::Block { reason } => (Kind::PostBlock, Some(reason), None, None),
      Decision::RuntimeFailure { reason, code } => {
        (Kind::RuntimeFailure, Some(reason), None, Some(code))
      }
    };
    Wire {
      kind,
      reason,
      message,
      code,
    }
  }
}

#[cfg(test)]
#[path = "tests/decision_test.rs"]
mod tests;
