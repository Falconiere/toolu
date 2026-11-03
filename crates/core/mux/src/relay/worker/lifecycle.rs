//! Trust and remove toolu-owned attempt directories on a worker.

use super::{Machine, Target, Transfer, error};
use toolu_runtime::env::Env;

/// Trust the attempt worktree and its clone for `agent` through the worker's
/// own documented trust stores, so the launch never waits at a folder dialog.
/// Hosts whose launch flags already trust the workspace need nothing here.
pub(super) fn trust(
  checkout: &Transfer<'_>,
  binary: &str,
  agent: &str,
  repo: &str,
) -> Result<(), String> {
  if !matches!(agent, "claude" | "codex") {
    return Ok(());
  }
  checkout
    .exec(
      &[
        binary,
        "delegate",
        "trust",
        "--agent",
        agent,
        "--path",
        checkout.target.repo,
        "--repo",
        repo,
      ],
      &[],
    )
    .map(drop)
    .map_err(|e| error(&e))
}

/// Remove a settled attempt's worktree from its clone and prune the clone's
/// worktree list. An attempt whose worktree is already gone needs nothing.
pub(in crate::relay) fn cleanup(
  env: &Env,
  machine: &Machine,
  attempt: &str,
  cwd: &str,
) -> Result<(), String> {
  let root = machine.root.trim_end_matches('/');
  let transfer = Transfer {
    target: Target {
      machine: &machine.name,
      repo: root,
    },
    env,
    run: attempt,
  };
  if !super::clone::exists(&transfer, &format!("{root}/tasks"), attempt)? {
    return Ok(());
  }
  let common = transfer
    .exec(
      &[
        "git",
        "-C",
        cwd,
        "rev-parse",
        "--path-format=absolute",
        "--git-common-dir",
      ],
      &[],
    )
    .map_err(|e| error(&e))?;
  let common = String::from_utf8(common)
    .map_err(|e| e.to_string())?
    .trim_end()
    .to_owned();
  let git = |args: &[&str]| {
    let mut argv = vec!["git", "--git-dir", common.as_str()];
    argv.extend_from_slice(args);
    transfer.exec(&argv, &[]).map(drop).map_err(|e| error(&e))
  };
  git(&["worktree", "remove", "--force", cwd])?;
  git(&["worktree", "prune"])
}

#[cfg(test)]
#[path = "tests/lifecycle_test.rs"]
mod tests;
