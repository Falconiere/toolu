//! Verify a sub-issue PR against the epic merge policy and optionally merge it
//! (`merge-gate.ts`), then close and tick the delivered item and record the
//! verdict on the issue. This is the manual gate; the engine's merge queue
//! (`queue_step`) reuses its verdicts, auto-merge and merge.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{Map, Value, json};
use toolu_engine::resources::lock::acquire_lock;

pub(crate) mod assess;
pub(crate) mod auto;
pub(crate) mod merge;
mod settle;

pub(crate) use assess::Pr;
pub(crate) use settle::{MakeTracker, Settle, settle_issue};

use crate::launch::record::{Record, write_record};
use crate::refs::parse_ref;

/// The flags of one gate run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct GateOpts {
  /// The PR reference.
  pub pr: String,
  /// The sub-issue to settle once merged.
  pub issue: Option<String>,
  /// The epic to tick.
  pub epic: Option<String>,
  /// Merge on a `merge` verdict.
  pub merge: bool,
  /// Arm auto-merge on a `wait` verdict.
  pub auto: bool,
  /// The merge method; otherwise the repository's first allowed.
  pub method: Option<String>,
  /// The epic state directory whose record gets `last_gate`.
  pub state_dir: Option<PathBuf>,
  /// The issue key of that record.
  pub key: Option<String>,
  /// Assess only: merge, arm, settle and record nothing.
  pub dry_run: bool,
}

/// The record stage after a verdict.
fn stage_after(verdict: &str) -> &'static str {
  if verdict == "wait" {
    "awaiting_merge"
  } else {
    "running"
  }
}

/// Run the gate on `opts.pr`.
///
/// # Errors
/// A bad reference, or a GitHub or tracker failure.
pub(crate) fn run_gate(opts: &GateOpts, at: &Settle<'_>) -> Result<Value, String> {
  let found = parse_ref(&opts.pr, None)?;
  let pr = Pr {
    owner: found.owner,
    repo: found.repo,
    number: found.number,
  };
  let (mut result, facts) = assess::assess(at.gh, &pr)?;
  let verdict = result
    .get("verdict")
    .and_then(Value::as_str)
    .unwrap_or("")
    .to_owned();
  let head = result
    .get("head")
    .and_then(Value::as_str)
    .unwrap_or("")
    .to_owned();
  if opts.dry_run {
    result.insert("dry_run".to_owned(), json!(true));
    return Ok(Value::Object(result));
  }
  if opts.merge && verdict == "merge" {
    let method = merge::merge_method(at.gh, &pr, opts.method.as_deref())?;
    let target = merge::Target {
      pr: &pr,
      facts: &facts,
      head: &head,
      method: &method,
    };
    result.extend(merge::do_merge(at.gh, &target));
  }
  apply_auto(opts, at, (&pr, &facts, &head), &mut result)?;
  settle_merged(opts, at, &verdict, &mut result)?;
  // The merge and settlement already happened: a record that cannot be
  // written is reported with the result, not instead of it.
  if let (Some(dir), Some(key)) = (&opts.state_dir, &opts.key)
    && let Err(err) = record_gate(dir, key, &result, &verdict, LOCK_WAIT)
  {
    result.insert("record_error".to_owned(), json!(err));
  }
  Ok(Value::Object(result))
}

/// Close and tick the delivered issue once its PR merged.
fn settle_merged(
  opts: &GateOpts,
  at: &Settle<'_>,
  verdict: &str,
  result: &mut Map<String, Value>,
) -> Result<(), String> {
  let merged = result.get("merged") == Some(&Value::Bool(true)) || verdict == "merged";
  let Some(issue) = opts.issue.as_deref().filter(|_| merged) else {
    return Ok(());
  };
  let graph = opts
    .state_dir
    .as_ref()
    .and_then(|dir| crate::graph::read_graph(&dir.join("graph.json")).ok());
  let url = result
    .get("url")
    .and_then(Value::as_str)
    .unwrap_or("")
    .to_owned();
  result.extend(settle::settle_issue(
    at,
    issue,
    &url,
    opts.epic.as_deref(),
    graph.as_ref(),
  )?);
  Ok(())
}

/// Arm or disarm auto-merge as the verdict asks.
fn apply_auto(
  opts: &GateOpts,
  at: &Settle<'_>,
  target: (&Pr, &assess::Facts, &str),
  result: &mut Map<String, Value>,
) -> Result<(), String> {
  let (pr, facts, head) = target;
  let verdict = result
    .get("verdict")
    .and_then(Value::as_str)
    .unwrap_or("")
    .to_owned();
  let armed = result.get("auto_merge_armed") == Some(&Value::Bool(true));
  match auto::action(&verdict, armed, opts.auto) {
    Some(auto::Action::Arm) => {
      let method = merge::merge_method(at.gh, pr, opts.method.as_deref())?;
      result.extend(auto::arm(at.gh, facts, head, &method));
    }
    Some(auto::Action::Disarm) => {
      auto::disarm(at.gh, facts)?;
      result.insert("auto_merge_disarmed".to_owned(), json!(true));
    }
    None => {}
  }
  Ok(())
}

/// Write the verdict's stage and `last_gate` onto `issues/<key>.json`, under
/// the per-issue lock a launch or finish takes, waiting up to `wait` for it.
fn record_gate(
  dir: &Path,
  key: &str,
  result: &Map<String, Value>,
  verdict: &str,
  wait: Duration,
) -> Result<(), String> {
  let lock = acquire_lock(&dir.join("launch-locks").join(key), wait)?
    .ok_or_else(|| format!("{key}: a launch or finish holds its record; gate again"))?;
  let written = write_gate(dir, key, result, verdict);
  lock.release();
  written
}

/// How long the gate waits for a launch or finish to let go of the record.
const LOCK_WAIT: Duration = Duration::from_secs(10);

fn write_gate(
  dir: &Path,
  key: &str,
  result: &Map<String, Value>,
  verdict: &str,
) -> Result<(), String> {
  let path = dir.join("issues").join(format!("{key}.json"));
  let mut record: Record = match crate::disk::read_value(&path) {
    Ok(Value::Object(record)) => record,
    _ => Record::new(),
  };
  let field = |name: &str| result.get(name).cloned().unwrap_or(Value::Null);
  let last = json!({
    "verdict": field("verdict"), "reasons": field("reasons"), "head": field("head"),
    "merged": field("merged"), "admin_used": field("admin_used"), "uncertain": field("uncertain"),
    "native_exit_code": field("native_exit_code"), "native_error_code": field("native_error_code"),
    "error": field("error"), "auto_merge": auto::state(result),
  });
  // Only a worker's own stages move: a record the engine already moved to
  // cleaning or merged during a slow settle keeps that stage.
  let stage = record.get("stage").and_then(Value::as_str).unwrap_or("");
  if matches!(stage, "" | "running" | "awaiting_merge") {
    record.insert("stage".to_owned(), json!(stage_after(verdict)));
  }
  record.insert("last_gate".to_owned(), last);
  write_record(&path, &record)
}

#[cfg(test)]
#[path = "tests/gate_test.rs"]
mod tests;
