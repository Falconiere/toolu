//! The no-CI merge policy (`toolu epic start --no-babysit`, #832): the
//! worker's local gate stands in for required checks. A PR whose checks are
//! the only thing pending merges when the worker's worktree sits at the PR
//! head and its plan ledger is fresh-green there, judged by the same plan
//! gate the push hook runs; otherwise it waits, naming the missing evidence.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};
use toolu_engine::verdict::gates::GateContext;
use toolu_engine::verdict::plan::plan_gate;
use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::json::ordered::Ordered;
use toolu_runtime::process::{Spec, run};
use toolu_state::diff_sha::diff_sha;
use toolu_state::git::{base_branch, current_branch};

/// Where the worker's local gate evidence lives.
#[derive(Debug, Clone, PartialEq, Eq)]
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
/// gate's verdict. Rebase, fix, merged and closed stand; so does a wait on
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
  let (verdict, reason) = match evidence(local, &field("head"), env) {
    Ok(reason) => ("merge", reason),
    Err(reason) => ("wait", reason),
  };
  result.insert("verdict".to_owned(), json!(verdict));
  result.insert("reasons".to_owned(), json!([reason]));
  result.insert("local_gate".to_owned(), json!(verdict == "merge"));
}

fn git_head(env: &Env, worktree: &Path) -> Option<String> {
  let mut spec = Spec::new(["git", "-C"]);
  spec.argv.push(worktree.display().to_string());
  spec.argv.extend(["rev-parse", "HEAD"].map(str::to_owned));
  spec.env = Some(env.clone());
  run(&spec)
    .ok()
    .filter(|out| out.exit_code == 0)
    .map(|out| out.stdout.trim().to_owned())
}

/// The plan gate's reason for `head` being merge-ready, or why it is not.
fn evidence(local: &LocalGate, head: &str, env: &Env) -> Result<String, String> {
  let worktree = local
    .worktree
    .as_deref()
    .ok_or("local gate: no worktree recorded")?;
  let host = local
    .host
    .as_deref()
    .and_then(Host::parse)
    .ok_or("local gate: no host recorded")?;
  let at = git_head(env, worktree)
    .ok_or_else(|| format!("local gate: no git HEAD in {}", worktree.display()))?;
  if at != head {
    return Err(format!(
      "local gate: worktree at {}, pull request head {}",
      short(&at),
      short(head)
    ));
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
    Ok(format!("local gate on {}: {}", short(head), text("reason")))
  } else {
    Err(format!("local gate: {}", text("reason")))
  }
}

fn short(sha: &str) -> &str {
  sha.get(..7).unwrap_or(sha)
}

#[cfg(test)]
#[path = "tests/local_test.rs"]
mod tests;
