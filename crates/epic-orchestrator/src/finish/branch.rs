//! Delete the issue's local branch once its worktree is gone (`deleteBranch`).

use std::time::Duration;

use serde_json::Value;
use toolu_runtime::env::Env;
use toolu_runtime::process::{Spec, run};

use crate::launch::record::{Record, text};

fn git(env: &Env, args: &[&str]) -> Option<i32> {
  let mut spec = Spec::new(["git"].into_iter().chain(args.iter().copied()));
  spec.env = Some(env.clone());
  spec.timeout = Duration::from_secs(30);
  run(&spec)
    .ok()
    .filter(|out| !out.timed_out && !out.truncated)
    .map(|out| out.exit_code)
}

/// Whether the branch is gone: already recorded deleted, absent, or deleted
/// now. A kept worktree or an abandon keeps it.
///
/// # Errors
/// git cannot say whether the branch exists.
pub(crate) fn delete_branch(
  record: &Record,
  removed: bool,
  abandon: bool,
  env: &Env,
) -> Result<bool, String> {
  if record.get("branch_deleted") == Some(&Value::Bool(true)) {
    return Ok(true);
  }
  if !removed || abandon {
    return Ok(false);
  }
  let (checkout, branch) = (
    text(record, "checkout").unwrap_or(""),
    text(record, "branch").unwrap_or(""),
  );
  let reference = format!("refs/heads/{branch}");
  match git(
    env,
    &[
      "-C", checkout, "show-ref", "--verify", "--quiet", &reference,
    ],
  ) {
    Some(1) => Ok(true),
    Some(0) => Ok(git(env, &["-C", checkout, "branch", "-D", branch]) == Some(0)),
    Some(_) | None => Err(format!("cannot verify local branch {reference}")),
  }
}

#[cfg(test)]
#[path = "tests/branch_test.rs"]
mod tests;
