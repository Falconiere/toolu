//! git in the worker's worktree for the mechanical rebase, and the checks
//! that must hold before the engine may touch it.

use std::path::Path;
use std::time::Duration;

use toolu_runtime::env::Env;
use toolu_runtime::process::{Spec, run};

use super::Target;

/// Fetch, rebase and push get this long.
const TIMEOUT: Duration = Duration::from_secs(120);
/// More output than this is a failure.
const MAX_OUTPUT: usize = 1 << 20;

/// git with `-C <worktree>` and the engine's environment.
pub(super) struct Git<'a> {
  worktree: String,
  env: &'a Env,
}

impl<'a> Git<'a> {
  /// git in `worktree`.
  pub(super) fn new(worktree: &Path, env: &'a Env) -> Self {
    Self {
      worktree: worktree.display().to_string(),
      env,
    }
  }

  /// Trimmed stdout, or the first line of stderr (or the exit) on failure.
  pub(super) fn out(&self, args: &[&str]) -> Result<String, String> {
    let line = ["git", "-C", self.worktree.as_str()]
      .into_iter()
      .chain(args.iter().copied());
    let mut spec = Spec::new(line);
    spec.env = Some(self.env.clone());
    spec.timeout = TIMEOUT;
    spec.max_output_bytes = MAX_OUTPUT;
    let out = run(&spec).map_err(|err| format!("git did not run: {err:?}"))?;
    if out.timed_out {
      return Err(format!("git {} timed out", args.first().unwrap_or(&"")));
    }
    if out.exit_code != 0 || out.truncated {
      let detail = out.stderr.trim();
      return Err(if detail.is_empty() {
        format!(
          "git {} exited {}",
          args.first().unwrap_or(&""),
          out.exit_code
        )
      } else {
        detail.to_owned()
      });
    }
    Ok(out.stdout.trim().to_owned())
  }

  /// Success or the failure text.
  pub(super) fn ok(&self, args: &[&str]) -> Result<(), String> {
    self.out(args).map(|_| ())
  }
}

/// The worktree is on the PR's branch at the gate's head, with no change to
/// a tracked file and no rebase or merge in progress.
pub(super) fn preconditions(git: &Git<'_>, target: &Target) -> Result<(), String> {
  if target.branch.is_empty() || target.branch == target.base {
    return Err(format!("refusing to rebase branch {:?}", target.branch));
  }
  let branch = git.out(&["rev-parse", "--abbrev-ref", "HEAD"])?;
  if branch != target.branch {
    return Err(format!("worktree is on {branch}, not {}", target.branch));
  }
  let head = git.out(&["rev-parse", "HEAD"])?;
  if head != target.head {
    return Err(format!(
      "local HEAD {head} is not the PR head {}",
      target.head
    ));
  }
  let status = git.out(&["status", "--porcelain", "--untracked-files=no"])?;
  if !status.is_empty() {
    return Err("uncommitted changes to tracked files".to_owned());
  }
  for marker in ["rebase-merge", "rebase-apply", "MERGE_HEAD"] {
    if in_progress(git, marker)? {
      return Err(format!("a {marker} is in progress"));
    }
  }
  Ok(())
}

/// Whether git's `marker` (a rebase directory, `MERGE_HEAD`) exists.
fn in_progress(git: &Git<'_>, marker: &str) -> Result<bool, String> {
  let path = git.out(&["rev-parse", "--git-path", marker])?;
  Ok(Path::new(&git.worktree).join(path).exists())
}

/// Whether a rebase is in progress.
pub(super) fn rebasing(git: &Git<'_>) -> Result<bool, String> {
  Ok(in_progress(git, "rebase-merge")? || in_progress(git, "rebase-apply")?)
}

/// Point `refs/epic-wip/<key>` at `head` (with a reflog entry).
pub(super) fn pin_head(git: &Git<'_>, key: &str, head: &str) -> Result<(), String> {
  let reference = format!("refs/epic-wip/{key}");
  git
    .ok(&[
      "update-ref",
      "--create-reflog",
      "-m",
      "epic rebase",
      &reference,
      head,
    ])
    .map_err(|reason| format!("snapshot failed: {reason}"))
}

#[cfg(test)]
#[path = "tests/git_test.rs"]
mod tests;
