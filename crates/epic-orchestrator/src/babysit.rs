//! The engine's babysit step: one tick through the [`BabysitTick`] that
//! `crates/cli` passes in (pr-babysit's), turned into what the engine does
//! next. A tick that fails is an attention item, never a crash.

use toolu_engine::LinkError;
use toolu_engine::babysit::{BabysitTick, GraphQlUsage, TickDecision, TickRequest};

use crate::journal::Record;
use crate::model::{Step, World};

/// What the engine does after one babysit tick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Next {
  /// Check again at the next interval.
  KeepGoing,
  /// Everything is green: queue the pull request for merge.
  MergeQueue,
  /// Raise an attention item with this message.
  Attention(String),
  /// Send the worker this prompt.
  Prompt(String),
  /// Raise an attention item with `note` and send the worker `prompt`.
  Escalate {
    /// The attention item's message.
    note: String,
    /// The worker's prompt.
    prompt: String,
  },
}

/// A babysit decision with the GraphQL usage supplied by the tick.
pub(crate) struct Checked {
  /// The state-machine input.
  pub next: Next,
  /// Separate GraphQL budget observation.
  pub graphql: Option<GraphQlUsage>,
  /// Rate limit returned by the tick, with no GraphQL result document.
  pub throttle: Option<Throttle>,
}

/// Client-neutral throttle metadata from a babysit tick.
pub(crate) struct Throttle {
  /// GitHub's requested wait.
  pub retry_after: Option<std::time::Duration>,
  /// GraphQL primary points left.
  pub remaining: Option<u64>,
  /// GraphQL primary reset as Unix seconds.
  pub reset_at: Option<u64>,
}

/// Run one tick of `request`'s pull request and decide the engine's next step.
pub fn next(tick: &dyn BabysitTick, request: &TickRequest) -> Next {
  check(tick, request).next
}

/// Run the full tick, preserving its GraphQL usage for the engine journal.
pub(crate) fn check(tick: &dyn BabysitTick, request: &TickRequest) -> Checked {
  let slot = format!("{}#{}", request.repo, request.number);
  match tick.tick(request) {
    Ok(report) => Checked {
      next: match report.decision {
        TickDecision::KeepGoing => Next::KeepGoing,
        TickDecision::Success => Next::MergeQueue,
        TickDecision::Escalate => match report.prompt {
          Some(prompt) => Next::Escalate {
            note: format!("babysit escalated {slot}"),
            prompt,
          },
          None => Next::Attention(format!("babysit escalated {slot}")),
        },
        TickDecision::Act => match report.prompt {
          Some(text) => Next::Prompt(text),
          None => Next::Attention(format!("babysit needs the worker on {slot}")),
        },
      },
      graphql: report.graphql,
      throttle: None,
    },
    Err(LinkError::RateLimited {
      retry_after,
      remaining,
      reset_at,
    }) => Checked {
      next: Next::Attention(format!("babysit tick for {slot} was rate limited")),
      graphql: None,
      throttle: Some(Throttle {
        retry_after,
        remaining,
        reset_at,
      }),
    },
    Err(err) => Checked {
      next: Next::Attention(format!("babysit tick for {slot} failed: {err}")),
      graphql: None,
      throttle: None,
    },
  }
}

/// Journal `text` as a `prompt` record, then send it to the worker. An
/// undelivered prompt raises a `fallback` attention item, if one is named.
pub(crate) fn prompt(
  world: &mut World,
  key: &str,
  name: &str,
  text: String,
  fallback: Option<&str>,
) {
  world
    .outbox
    .push_back(Step::Journal(Record::new("prompt", name, key, "", &text)));
  world.outbox.push_back(Step::Prompt {
    key: key.to_owned(),
    text,
    fallback: fallback.map(str::to_owned),
  });
}

#[cfg(test)]
#[path = "tests/babysit_test.rs"]
mod tests;
