//! The in-process cleanup effect: finish the merged issue (not `--abandon`),
//! retrying with backoff while its agent or workload is still exiting, then
//! settle its tracker item (close and tick) unless the tracker already
//! reports it closed. The outcome is one JSON document for `commit`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{Value, json};
use toolu_runtime::env::Env;

use crate::executor::Job;
use crate::finish::{FinishReport, finish_issue, still_exiting};
use crate::gate::{MakeTracker, Settle};
use crate::graph::read_graph;
use crate::launch::Ctx;
use crate::paths::Paths;
use crate::reach::{Reach, redact};
use crate::retry::{Policy, effect_scope, number};
use crate::tracker::{Gh, TrackerKind, detect_tracker};

/// What one cleanup needs, moved onto the effect thread.
pub(crate) struct CleanupJob {
  /// The epic state directory.
  pub state_dir: PathBuf,
  /// The issue key.
  pub key: String,
  /// The environment effects run with.
  pub env: Env,
  /// The engine's resource root, for herdr.
  pub paths: Paths,
  /// The client builders.
  pub reach: Reach,
}

/// The cleanup effect body.
pub(crate) fn job(input: CleanupJob) -> Job {
  Box::new(move || {
    let policy = Policy::from_env(&input.env);
    effect_scope(&policy, || cleanup(&input)).to_string()
  })
}

/// How often a teardown whose agent or workload is still exiting is tried.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CleanupRetry {
  /// Attempts, the first included.
  pub attempts: u32,
  /// The pause before the second attempt; it doubles after each one.
  pub backoff: Duration,
}

impl CleanupRetry {
  /// `EPIC_CLEANUP_ATTEMPTS` (3, at least 1), `EPIC_CLEANUP_BACKOFF_MS` (5000).
  pub(crate) fn from_env(env: &Env) -> Self {
    let attempts = u32::try_from(number(env, "EPIC_CLEANUP_ATTEMPTS", 3)).unwrap_or(3);
    Self {
      attempts: attempts.max(1),
      backoff: Duration::from_millis(number(env, "EPIC_CLEANUP_BACKOFF_MS", 5000)),
    }
  }
}

/// Finish the merged issue, trying again with backoff while its agent or
/// workload is still exiting; any other failure ends it at once.
///
/// # Errors
/// The last attempt's reason (prefixed with the attempt count once
/// retried), or a failure no retry can clear.
pub(crate) fn finish_retrying(
  state_dir: &Path,
  key: &str,
  ctx: &mut Ctx<'_>,
  retry: CleanupRetry,
) -> Result<FinishReport, String> {
  let (mut attempt, mut pause) = (1, retry.backoff);
  loop {
    match finish_issue(state_dir, key, (false, false), ctx) {
      Err(reason) if still_exiting(&reason) && attempt < retry.attempts => {
        std::thread::sleep(pause);
        pause = pause.saturating_mul(2);
        attempt += 1;
      }
      Err(reason) if still_exiting(&reason) && attempt > 1 => {
        return Err(format!("after {attempt} attempts: {reason}"));
      }
      other => return other,
    }
  }
}

/// Finish, then settle: `applied` (with `settle` or `settle_error`) or
/// `cleanup-incomplete` with the reason.
pub(crate) fn cleanup(input: &CleanupJob) -> Value {
  let gh = (input.reach.gh)(&input.env);
  let mut ctx = Ctx::live(&input.env, &input.paths, gh.as_ref().map_err(Clone::clone));
  let retry = CleanupRetry::from_env(&input.env);
  let report = match finish_retrying(&input.state_dir, &input.key, &mut ctx, retry) {
    Ok(report) => report,
    Err(err) => return incomplete(&input.env, &err),
  };
  if report.finished.get("stage").and_then(Value::as_str) != Some("merged") {
    return incomplete(&input.env, &unmerged_note(&report));
  }
  match settle(input, gh.as_ref().map_err(Clone::clone)) {
    Ok(settled) => json!({"outcome": "applied", "settle": settled, "tracker_closed": true}),
    Err(err) => json!({"outcome": "applied", "settle_error": redact(&input.env, &err)}),
  }
}

/// Why a finish that returned stopped short of `merged`: its warnings (a
/// failed workspace removal), or a pointer to the record.
fn unmerged_note(report: &FinishReport) -> String {
  if report.warnings.is_empty() {
    "cleanup incomplete; see the issue record".to_owned()
  } else {
    format!("cleanup incomplete: {}", report.warnings.join("; "))
  }
}

fn incomplete(env: &Env, reason: &str) -> Value {
  json!({"outcome": "cleanup-incomplete", "note": redact(env, reason)})
}

/// Close and tick the issue's tracker item unless it is already closed.
fn settle(input: &CleanupJob, gh: Result<&Gh, String>) -> Result<Value, String> {
  let graph = read_graph(&input.state_dir.join("graph.json"))?;
  let issue = graph
    .find(&input.key)
    .ok_or_else(|| format!("{} is not in the epic graph", input.key))?;
  let kind = detect_tracker(&graph.epic.reference, graph.tracker.as_deref(), &input.env)?;
  let repo = graph.default_repo.clone().unwrap_or_default();
  let tracker = (input.reach.tracker)(kind, &graph.epic.reference, &repo, &input.env)?;
  if tracker.is_closed(&issue.reference)? {
    return Ok(json!({"issue": issue.reference, "already_closed": true}));
  }
  let pr_url = pr_url(&input.state_dir, &input.key, &issue.repo)?;
  let make: &MakeTracker<'_> = &|kind: TrackerKind, reference: &str, repo: &str| {
    (input.reach.tracker)(kind, reference, repo, &input.env)
  };
  let at = Settle {
    env: &input.env,
    gh: gh?,
    make,
  };
  crate::gate::settle_issue(
    &at,
    &issue.reference,
    &pr_url,
    Some(&graph.epic.reference),
    (Some(&graph), &input.state_dir),
  )
  .map(Value::Object)
}

/// The pull request the worker reported for `key`.
fn pr_url(state_dir: &Path, key: &str, repo: &str) -> Result<String, String> {
  let status = crate::disk::read_value(&state_dir.join("status").join(format!("{key}.json")))?;
  match status.get("pr") {
    Some(Value::Number(number)) => Ok(format!("https://github.com/{repo}/pull/{number}")),
    _ => Err(format!("no PR recorded for {key}")),
  }
}

#[cfg(test)]
#[path = "tests/effects_cleanup_test.rs"]
mod tests;
