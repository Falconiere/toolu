//! The no-CI merge policy (`toolu epic start --no-babysit`, #832): the
//! worker's local gate stands in for required checks. A PR whose checks are
//! the only thing pending merges when the worker's worktree sits at the PR
//! head and its plan ledger is fresh-green there, judged by the same plan
//! gate the push hook runs; otherwise it waits, naming the missing evidence.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};
use toolu_engine::verdict::gates::GateContext;
use toolu_engine::verdict::plan::plan_gate;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::json::ordered::Ordered;
use toolu_runtime::process::{Spec, run};
use toolu_state::diff_sha::diff_sha;
use toolu_state::git::{base_branch, current_branch};

use crate::hosts::ledger_host;

/// Where the worker's local gate evidence lives.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct LocalGate {
  /// The worker's worktree, when recorded.
  pub worktree: Option<PathBuf>,
  /// The host the worker was launched on; its ledger lives under that host's dir.
  pub host: Option<String>,
}

/// The policy of the epic at `state_dir`, with the launch record's
/// `worktree` and `kind`; `None` when babysit is on.
pub(crate) fn policy(state_dir: &Path, record: &Value, fallback: &LocalGate) -> Option<LocalGate> {
  let text = |key: &str| {
    record
      .get(key)
      .and_then(Value::as_str)
      .filter(|text| !text.is_empty())
  };
  crate::epics::launch_settings(state_dir)
    .no_babysit
    .then(|| LocalGate {
      worktree: text("worktree")
        .map(PathBuf::from)
        .or_else(|| fallback.worktree.clone()),
      host: text("kind")
        .map(str::to_owned)
        .or_else(|| fallback.host.clone()),
    })
}

/// Replace a checks-only `wait` or a checks-backed `merge` with the local
/// gate's verdict: `merge` on a fresh-green ledger at the pull request head,
/// else `wait` naming the missing evidence, with `local_wait` set when the
/// worker owes it. Rebase, fix, merged and closed stand; so does a wait on
/// GitHub still computing mergeability.
pub(crate) fn apply(result: &mut Map<String, Value>, local: &LocalGate, env: &Env) {
  let field = |key: &str| {
    result
      .get(key)
      .and_then(Value::as_str)
      .unwrap_or("")
      .to_owned()
  };
  if !matches!(field("verdict").as_str(), "wait" | "merge") || field("mergeable") == "UNKNOWN" {
    return;
  }
  let (verdict, reason) = evidence(local, &field("head"), env);
  result.insert(
    "verdict".to_owned(),
    json!(if verdict == "merge" { "merge" } else { "wait" }),
  );
  result.insert("reasons".to_owned(), json!([reason]));
  result.insert("local_gate".to_owned(), json!(verdict == "merge"));
  result.insert("local_wait".to_owned(), json!(verdict == "fix"));
}

fn git_head(env: &Env, worktree: &Path) -> Option<String> {
  let mut spec = Spec::new(["git", "rev-parse", "HEAD"]);
  spec.cwd = Some(worktree.to_path_buf());
  spec.env = Some(env.clone());
  run(&spec)
    .ok()
    .filter(|out| out.exit_code == 0)
    .map(|out| out.stdout.trim().to_owned())
}

/// The local gate's verdict for `head` with its reason: `merge`, `fix` for
/// evidence only the worker can produce (its worktree elsewhere, its ledger
/// missing or stale), `wait` for what the engine itself lacks. Nothing else
/// prompts an idle worker under `--no-babysit`, and a waiting head blocks its
/// repository's queue, so the queue step turns a `fix` into a prompt. The
/// engine's own rebase stales the ledger (`diff_sha` hashes against the local
/// base branch, which only the worker's next gate run catches up with), so
/// this is the common path, not an error.
fn evidence(local: &LocalGate, head: &str, env: &Env) -> (&'static str, String) {
  let Some(worktree) = local.worktree.as_deref() else {
    return ("wait", "local gate: no worktree recorded".to_owned());
  };
  let Some(host) = local.host.as_deref().and_then(ledger_host) else {
    return ("wait", "local gate: no host recorded".to_owned());
  };
  let Some(at) = git_head(env, worktree) else {
    let reason = format!("local gate: no git HEAD in {}", worktree.display());
    return ("wait", reason);
  };
  if at != head {
    let reason = format!(
      "local gate: worktree at {}, pull request head {}",
      short(&at),
      short(head)
    );
    return ("fix", reason);
  }
  let base = base_branch(env, Some(worktree), worktree);
  let ctx = GateContext {
    roots: &Roots::new(env.clone(), Some(host)),
    root: worktree.to_path_buf(),
    branch: current_branch(env, worktree),
    cur: diff_sha(env, worktree, &base).unwrap_or_default(),
    base,
    cwd: worktree.to_path_buf(),
    warnings: Vec::new(),
  };
  let gate = plan_gate(&ctx);
  let text = |key: &str| match gate.get(key) {
    Some(Ordered::String(text)) => text.clone(),
    _ => String::new(),
  };
  if text("state") == "pass" {
    (
      "merge",
      format!("local gate on {}: {}", short(head), text("reason")),
    )
  } else {
    ("fix", format!("local gate: {}", text("reason")))
  }
}

fn short(sha: &str) -> &str {
  sha.get(..7).unwrap_or(sha)
}

#[cfg(test)]
#[path = "tests/local_test.rs"]
mod tests;
