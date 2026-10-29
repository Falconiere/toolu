//! git in the worker's worktree for the mechanical rebase, and the checks
//! that must hold before the engine may touch it.

use std::path::Path;
use std::time::Duration;

use serde_json::Value;
use toolu_runtime::env::Env;
use toolu_runtime::process::{Output, Spec, run};

use super::Target;

/// Fetch, rebase and push get this long.
const TIMEOUT: Duration = Duration::from_secs(120);
/// More output than this is a failure.
const MAX_OUTPUT: usize = 1 << 20;

/// git with `-C <worktree>` and the engine's environment.
pub(super) struct Git<'a> {
  worktree: String,
  env: &'a Env,
  timeout: Duration,
}

impl<'a> Git<'a> {
  /// git in `worktree`.
  pub(super) fn new(worktree: &Path, env: &'a Env) -> Self {
    Self::with_timeout(worktree, env, TIMEOUT)
  }

  /// Git with a bounded command deadline, shortened for fault tests.
  pub(super) fn with_timeout(worktree: &Path, env: &'a Env, timeout: Duration) -> Self {
    Self {
      worktree: worktree.display().to_string(),
      env,
      timeout,
    }
  }

  /// Trimmed stdout, or the first line of stderr (or the exit) on failure.
  pub(super) fn out(&self, args: &[&str]) -> Result<String, String> {
    let out = self.command(args, false)?;
    if out.timed_out {
      return Err(format!("git {} timed out", args.first().unwrap_or(&"")));
    }
    if out.exit_code != 0 || out.truncated {
      return Err(failure(args, &out));
    }
    Ok(out.stdout.trim().to_owned())
  }

  /// Push with a Git trace so a failed local hook can be distinguished from a lost reply.
  pub(super) fn push(&self, args: &[&str]) -> Result<(), (String, bool)> {
    let out = self.command(args, true).map_err(|reason| (reason, false))?;
    if out.timed_out {
      return Err(("git push timed out".to_owned(), false));
    }
    if out.exit_code == 0 && !out.truncated {
      return Ok(());
    }
    let refused = !out.truncated && pre_push_refused(&out.stderr);
    let stderr = out
      .stderr
      .lines()
      .filter(|line| !is_trace_event(line))
      .collect::<Vec<_>>()
      .join("\n");
    Err((failure_text(args, &stderr, out.exit_code), refused))
  }

  fn command(&self, args: &[&str], trace: bool) -> Result<Output, String> {
    let line = ["git", "-C", self.worktree.as_str()]
      .into_iter()
      .chain(args.iter().copied());
    let mut spec = Spec::new(line);
    spec.env = Some(if trace {
      self.env.clone().with("GIT_TRACE2_EVENT", "1")
    } else {
      self.env.clone()
    });
    spec.timeout = self.timeout;
    spec.max_output_bytes = MAX_OUTPUT;
    run(&spec).map_err(|err| format!("git did not run: {err:?}"))
  }

  /// Success or the failure text.
  pub(super) fn ok(&self, args: &[&str]) -> Result<(), String> {
    self.out(args).map(|_| ())
  }
}

fn failure(args: &[&str], out: &Output) -> String {
  failure_text(args, &out.stderr, out.exit_code)
}

fn failure_text(args: &[&str], stderr: &str, exit_code: i32) -> String {
  let detail = stderr.trim();
  if detail.is_empty() {
    format!("git {} exited {exit_code}", args.first().unwrap_or(&""))
  } else {
    detail.to_owned()
  }
}

fn is_trace_event(line: &str) -> bool {
  serde_json::from_str::<Value>(line)
    .is_ok_and(|event| event.get("event").is_some() && event.get("sid").is_some())
}

fn pre_push_refused(stderr: &str) -> bool {
  let mut root = None;
  let mut hook = None;
  for line in stderr.lines() {
    let Ok(event) = serde_json::from_str::<Value>(line) else {
      continue;
    };
    let kind = event.get("event").and_then(Value::as_str);
    let sid = event.get("sid").and_then(Value::as_str);
    let child_id = event.get("child_id").and_then(Value::as_u64);
    if root.is_none() && kind == Some("version") {
      root = sid.map(str::to_owned);
    }
    if root.as_deref() != sid {
      continue;
    }
    if kind == Some("child_start")
      && event.get("child_class").and_then(Value::as_str) == Some("hook")
      && event.get("hook_name").and_then(Value::as_str) == Some("pre-push")
    {
      hook = sid.zip(child_id).map(|(sid, id)| (sid.to_owned(), id));
    }
    if kind == Some("child_exit")
      && hook
        .as_ref()
        .is_some_and(|(hook_sid, id)| Some(hook_sid.as_str()) == sid && Some(*id) == child_id)
      && event
        .get("code")
        .and_then(Value::as_i64)
        .is_some_and(|code| code != 0)
    {
      return true;
    }
  }
  false
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
