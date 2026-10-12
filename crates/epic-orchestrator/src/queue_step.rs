//! The engine's step on the head of a merge queue (#448), run in process on
//! the effect thread: gate the PR, then act on the verdict. Behind → disarm
//! auto-merge and rebase it mechanically; checks running → arm auto-merge
//! pinned to the head; green → merge pinned to the head; failing → disarm
//! and hand back to babysit. The outcome is one JSON document for
//! `commit_queue`.

use std::path::PathBuf;

use serde_json::{Map, Value, json};
use toolu_runtime::env::Env;

use crate::executor::Job;
use crate::gate::assess::{Facts, Pr, assess};
use crate::gate::{auto, merge};
use crate::reach::{Reach, redact};
use crate::rebase::{Lease, Rebased, Target, rebase};
use crate::tracker::Gh;

/// What the step does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
  /// Work the queue head; `parked` when its worker may be rebased under.
  Head {
    /// Phase `ready`, agent not working, herdr reachable.
    parked: bool,
  },
  /// The issue left the queue: only disarm its auto-merge.
  Disarm,
}

/// What one step needs, moved onto the effect thread.
pub(crate) struct StepJob {
  /// `owner/repo`.
  pub repo: String,
  /// The pull request.
  pub pr: u64,
  /// The issue key (the snapshot ref).
  pub key: String,
  /// Head or disarm.
  pub mode: Mode,
  /// The worker's worktree, when recorded.
  pub worktree: Option<PathBuf>,
  /// The agent lease that fences it, when recorded.
  pub lease: Option<Lease>,
  /// The environment git and GitHub run with.
  pub env: Env,
  /// The GitHub client builder.
  pub reach: Reach,
  /// `GIT_TRACE2_EVENT` for the snapshot, in tests.
  pub trace: Option<PathBuf>,
}

/// The step as an effect body.
pub(crate) fn job(input: StepJob) -> Job {
  Box::new(move || step(&input).to_string())
}

fn outcome(name: &str) -> Map<String, Value> {
  let mut out = Map::new();
  out.insert("outcome".to_owned(), json!(name));
  out
}

fn with(mut out: Map<String, Value>, key: &str, value: Value) -> Map<String, Value> {
  out.insert(key.to_owned(), value);
  out
}

fn text(result: &Map<String, Value>, key: &str) -> String {
  result
    .get(key)
    .and_then(Value::as_str)
    .unwrap_or("")
    .to_owned()
}

/// Gate the PR and act on its verdict.
pub(crate) fn step(input: &StepJob) -> Value {
  let out = match run(input) {
    Ok(out) => out,
    Err(err) => with(outcome("error"), "note", json!(redact(&input.env, &err))),
  };
  Value::Object(out)
}

fn run(input: &StepJob) -> Result<Map<String, Value>, String> {
  let gh = (input.reach.gh)(&input.env)?;
  let (owner, repo) = input
    .repo
    .split_once('/')
    .ok_or_else(|| format!("bad repository {:?}", input.repo))?;
  let pr = Pr {
    owner: owner.to_owned(),
    repo: repo.to_owned(),
    number: input.pr,
  };
  let (result, facts) = assess(&gh, &pr)?;
  let armed = result.get("auto_merge_armed") == Some(&Value::Bool(true));
  let verdict = text(&result, "verdict");
  let head = text(&result, "head");
  let parked = match input.mode {
    Mode::Disarm => {
      disarm(&gh, &facts, armed)?;
      return Ok(outcome("disarmed"));
    }
    Mode::Head { parked } => parked,
  };
  let out = match verdict.as_str() {
    "merged" | "closed" => outcome(&verdict),
    "fix" => {
      disarm(&gh, &facts, armed)?;
      with(
        outcome("fix"),
        "reasons",
        result.get("reasons").cloned().unwrap_or(json!([])),
      )
    }
    "rebase" => {
      disarm(&gh, &facts, armed)?;
      rebased(input, &result, parked)
    }
    "wait" => waiting(&gh, &pr, &facts, &head, armed)?,
    "merge" => merged(&gh, &pr, &facts, &head, &input.env)?,
    other => return Err(format!("unknown verdict {other:?}")),
  };
  Ok(with(out, "head", json!(head)))
}

/// After a restart: `merged` when GitHub merged the PR, `open` to gate it
/// again, `failed` when GitHub cannot say.
pub(crate) fn reconcile(input: &StepJob) -> String {
  let state = (input.reach.gh)(&input.env).and_then(|gh| {
    let (owner, repo) = input.repo.split_once('/').unwrap_or_default();
    let pr = Pr {
      owner: owner.to_owned(),
      repo: repo.to_owned(),
      number: input.pr,
    };
    assess(&gh, &pr)
  });
  match state {
    Ok((result, _)) if text(&result, "verdict") == "merged" => "merged",
    Ok(_) => "open",
    Err(_) => "failed",
  }
  .to_owned()
}

fn disarm(gh: &Gh, facts: &Facts, armed: bool) -> Result<(), String> {
  if armed {
    auto::disarm(gh, facts)
  } else {
    Ok(())
  }
}

/// The mechanical rebase of a parked worker's worktree.
fn rebased(input: &StepJob, result: &Map<String, Value>, parked: bool) -> Map<String, Value> {
  if !parked {
    return with(outcome("busy"), "note", json!("worker active"));
  }
  let Some(worktree) = input.worktree.clone() else {
    return with(outcome("handoff"), "note", json!("no worktree recorded"));
  };
  let target = Target {
    worktree,
    key: input.key.clone(),
    branch: text(result, "head_ref"),
    base: text(result, "base"),
    head: text(result, "head"),
  };
  match rebase(
    &target,
    input.lease.as_ref(),
    &input.env,
    input.trace.as_deref(),
  ) {
    Rebased::Pushed(head) => with(outcome("rebased"), "new_head", json!(head)),
    Rebased::Handoff(note) => with(outcome("handoff"), "note", json!(redact(&input.env, &note))),
    Rebased::Busy(note) => with(outcome("busy"), "note", json!(redact(&input.env, &note))),
  }
}

/// Checks run: arm auto-merge pinned to the head, unless armed already.
fn waiting(
  gh: &Gh,
  pr: &Pr,
  facts: &Facts,
  head: &str,
  armed: bool,
) -> Result<Map<String, Value>, String> {
  if armed {
    return Ok(with(outcome("waiting"), "auto_merge", json!("armed")));
  }
  let method = merge::merge_method(gh, pr, None)?;
  let arm = auto::arm(gh, facts, head, &method);
  let state = if arm.get("auto_merge") == Some(&Value::Bool(true)) {
    "armed"
  } else {
    "unavailable"
  };
  Ok(with(outcome("waiting"), "auto_merge", json!(state)))
}

/// Green: merge pinned to the verified head.
fn merged(
  gh: &Gh,
  pr: &Pr,
  facts: &Facts,
  head: &str,
  env: &Env,
) -> Result<Map<String, Value>, String> {
  let method = merge::merge_method(gh, pr, None)?;
  let target = merge::Target {
    pr,
    facts,
    head,
    method: &method,
  };
  let done = merge::do_merge(gh, &target);
  let admin = done.get("admin_used") == Some(&Value::Bool(true));
  let error = redact(env, &text(&done, "error"));
  let status = done.get("native_exit_code").and_then(Value::as_u64);
  let name = if done.get("merged") == Some(&Value::Bool(true)) {
    "merged"
  } else if done.get("uncertain") == Some(&Value::Bool(true)) {
    "uncertain"
  } else if status == Some(409) || error.contains("Head branch was modified") {
    "stale"
  } else {
    "refused"
  };
  let out = with(outcome(name), "admin_used", json!(admin));
  Ok(if error.is_empty() {
    out
  } else {
    with(out, "note", json!(error))
  })
}

#[cfg(test)]
#[path = "tests/queue_step_test.rs"]
mod tests;
