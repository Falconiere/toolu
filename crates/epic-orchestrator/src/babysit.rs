//! The engine's babysit step: one tick through the [`BabysitTick`] that
//! `crates/cli` passes in (pr-babysit's), turned into what the engine does
//! next. A tick that fails is an attention item, never a crash.

use toolu_engine::babysit::{BabysitTick, TickDecision, TickRequest};

/// What the engine does after one babysit tick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Next {
  /// Check again at the next interval.
  KeepGoing,
  /// Everything is green: queue the pull request for merge.
  MergeQueue,
  /// Raise an attention item with this message.
  Attention(String),
}

/// Run one tick of `request`'s pull request and decide the engine's next step.
pub fn next(tick: &dyn BabysitTick, request: &TickRequest) -> Next {
  let slot = format!("{}#{}", request.repo, request.number);
  match tick.tick(request) {
    Ok(report) => match report.decision {
      TickDecision::KeepGoing => Next::KeepGoing,
      TickDecision::Success => Next::MergeQueue,
      TickDecision::Escalate => Next::Attention(format!("babysit escalated {slot}")),
    },
    Err(err) => Next::Attention(format!("babysit tick for {slot} failed: {err}")),
  }
}

#[cfg(test)]
#[path = "tests/babysit_test.rs"]
mod tests;
