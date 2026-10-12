//! The in-process cleanup effect: finish the merged issue (not `--abandon`),
//! then settle its tracker item (close and tick) unless the tracker already
//! reports it closed. The outcome is one JSON document for `commit`.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use toolu_runtime::env::Env;

use crate::executor::Job;
use crate::finish::finish_issue;
use crate::gate::{MakeTracker, Settle};
use crate::graph::read_graph;
use crate::launch::Ctx;
use crate::paths::Paths;
use crate::reach::{Reach, redact};
use crate::retry::{Policy, effect_scope};
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

/// Finish, then settle: `applied` (with `settle` or `settle_error`) or
/// `cleanup-incomplete` with the reason.
pub(crate) fn cleanup(input: &CleanupJob) -> Value {
  let gh = (input.reach.gh)(&input.env);
  let mut ctx = Ctx::live(&input.env, &input.paths, gh.as_ref().map_err(Clone::clone));
  let finished = match finish_issue(&input.state_dir, &input.key, (false, false), &mut ctx) {
    Ok(report) => report.finished,
    Err(err) => return incomplete(&input.env, &err),
  };
  if finished.get("stage").and_then(Value::as_str) != Some("merged") {
    return incomplete(&input.env, "cleanup incomplete; see the issue record");
  }
  match settle(input, gh.as_ref().map_err(Clone::clone)) {
    Ok(settled) => json!({"outcome": "applied", "settle": settled, "tracker_closed": true}),
    Err(err) => json!({"outcome": "applied", "settle_error": redact(&input.env, &err)}),
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
    Some(&graph),
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
