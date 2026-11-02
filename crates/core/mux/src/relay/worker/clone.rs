//! Recover interrupted clones without deleting existing worker data.

use super::{Machine, Task, Transfer, error};

pub(super) fn ensure(
  transfer: &Transfer<'_>,
  task: &Task,
  machine: &Machine,
  hash: &str,
  repo: &str,
) -> Result<(), String> {
  let root = machine.root.trim_end_matches('/');
  let repos = format!("{root}/repos");
  transfer
    .exec(&["mkdir", "-p", &repos, &format!("{root}/tasks")], &[])
    .map_err(|e| error(&e))?;
  if exists(transfer, &repos, hash)? {
    if valid(transfer, repo, &task.request.origin)? {
      return Ok(());
    }
    quarantine(transfer, repo)?;
  }
  let attempt = task.attempt.as_deref().ok_or("missing attempt")?;
  let stage_name = format!("{hash}.{attempt}.clone");
  let stage = prepare_stage(transfer, &repos, &stage_name, &task.request.origin)?;
  transfer
    .exec(&["mv", "-n", &stage, repo], &[])
    .map_err(|e| error(&e))?;
  if exists(transfer, &repos, &stage_name)? {
    return Err("repository appeared while publishing clone".into());
  }
  if !valid(transfer, repo, &task.request.origin)? {
    return Err("published clone is incomplete".into());
  }
  Ok(())
}

fn prepare_stage(
  transfer: &Transfer<'_>,
  repos: &str,
  stage_name: &str,
  origin: &str,
) -> Result<String, String> {
  let stage = format!("{repos}/{stage_name}");
  if exists(transfer, repos, stage_name)? {
    if has_worktrees(transfer, &stage)? {
      return Err("staged repository has registered worktrees".into());
    }
    if !valid(transfer, &stage, origin)? {
      quarantine(transfer, &stage)?;
    }
  }
  if !exists(transfer, repos, stage_name)? {
    transfer
      .exec(
        &["git", "clone", "--no-checkout", "--", origin, &stage],
        &[],
      )
      .map_err(|e| error(&e))?;
  }
  if !valid(transfer, &stage, origin)? {
    return Err("staged clone is incomplete".into());
  }
  Ok(stage)
}

fn exists(transfer: &Transfer<'_>, dir: &str, name: &str) -> Result<bool, String> {
  let listing = transfer
    .exec(&["ls", "-A", dir], &[])
    .map_err(|e| error(&e))?;
  let listing = std::str::from_utf8(&listing).map_err(|e| e.to_string())?;
  Ok(listing.lines().any(|line| line == name))
}

fn valid(transfer: &Transfer<'_>, repo: &str, origin: &str) -> Result<bool, String> {
  if !exists(transfer, repo, ".git")? {
    return Ok(false);
  }
  let git = format!("{repo}/.git");
  if has_worktrees(transfer, repo)?
    && (!exists(transfer, &git, "HEAD")? || !exists(transfer, &git, "config")?)
  {
    return Err("incomplete repository has registered worktrees".into());
  }
  if !exists(transfer, &git, "HEAD")? || !exists(transfer, &git, "config")? {
    return Ok(false);
  }
  let output = transfer
    .exec(&["git", "-C", repo, "rev-parse", "--show-prefix"], &[])
    .map_err(|e| error(&e))?;
  if !std::str::from_utf8(&output)
    .map_err(|e| e.to_string())?
    .trim_end()
    .is_empty()
  {
    return Err("existing repository root differs".into());
  }
  let output = transfer
    .exec(&["git", "-C", repo, "remote", "get-url", "origin"], &[])
    .map_err(|e| error(&e))?;
  if std::str::from_utf8(&output)
    .map_err(|e| e.to_string())?
    .trim_end()
    != origin
  {
    return Err("existing repository origin differs".into());
  }
  let inside = transfer
    .exec(
      &["git", "-C", repo, "rev-parse", "--is-inside-work-tree"],
      &[],
    )
    .map_err(|e| error(&e))?;
  if inside != b"true\n" {
    return Err("existing repository is not a worktree".into());
  }
  Ok(true)
}

fn has_worktrees(transfer: &Transfer<'_>, repo: &str) -> Result<bool, String> {
  if !exists(transfer, repo, ".git")? {
    return Ok(false);
  }
  let git = format!("{repo}/.git");
  if !exists(transfer, &git, "worktrees")? {
    return Ok(false);
  }
  let entries = transfer
    .exec(&["ls", "-A", &format!("{git}/worktrees")], &[])
    .map_err(|e| error(&e))?;
  Ok(!entries.is_empty())
}

fn quarantine(transfer: &Transfer<'_>, path: &str) -> Result<(), String> {
  let saved = format!("{path}.incomplete-{}", toolu_state::lock::token());
  transfer
    .exec(&["mv", "-n", path, &saved], &[])
    .map_err(|e| error(&e))?;
  Ok(())
}

#[cfg(test)]
#[path = "tests/clone_test.rs"]
mod tests;
