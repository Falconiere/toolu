//! pr-babysit's side of the epic engine's [`BabysitTick`]: one scheduled check
//! of a pull request, read once from GitHub, reported as a [`TickReport`].

use std::path::Path;
use std::time::{Duration, SystemTime};

use serde_json::Value;
use toolu_engine::LinkError;
use toolu_engine::babysit::{TickDecision, TickReport, TickRequest};
use toolu_runtime::env::Env;
use toolu_state::time::iso_seconds;

use crate::error::Failure;
use crate::tick::{self, Source};

/// The decision a tick's result document names.
fn decision(result: &Value) -> TickDecision {
  match result.get("decision").and_then(Value::as_str) {
    Some("success") => TickDecision::Success,
    Some("escalate") => TickDecision::Escalate,
    _ => TickDecision::KeepGoing,
  }
}

/// `failure` as the engine's error: a GitHub rate limit keeps its wait.
fn link_error(failure: &Failure) -> LinkError {
  let extra = |key: &str| failure.extra.get(key).and_then(Value::as_u64);
  if failure.extra.get("class").and_then(Value::as_str) == Some("rate_limited") {
    return LinkError::RateLimited {
      retry_after: extra("retryAfter").map(Duration::from_secs),
      remaining: extra("remaining"),
      reset_at: extra("resetAt"),
    };
  }
  LinkError::Failed(format!("{}: {}", failure.code, failure.message))
}

/// One scheduled check of `request`'s pull request, read from `source`.
pub(crate) fn tick(
  request: &TickRequest,
  source: Source<'_>,
  env: &Env,
) -> Result<TickReport, LinkError> {
  let now = request
    .now
    .clone()
    .unwrap_or_else(|| iso_seconds(SystemTime::now()));
  let run = tick::Request {
    repo: &request.repo,
    number: request.number,
    state_file: Path::new(&request.state_file),
    snapshot_out: None,
    now: &now,
    scheduled: true,
  };
  let ticked = tick::tick(&run, source, env).map_err(|failure| link_error(&failure))?;
  Ok(TickReport {
    decision: decision(&ticked.result),
    result: ticked.result,
    graphql: ticked.graphql,
  })
}

#[cfg(test)]
#[path = "tests/link_test.rs"]
mod tests;
