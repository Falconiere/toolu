//! Verify and advance only the recorded local mirror.

use super::{command, field};
use crate::launch::record::Record;
use toolu_runtime::env::Env;

pub(super) fn advance_mirror(record: &Record, env: &Env, commit: &str) -> Result<(), String> {
  let tree = field(record, "worktree")?;
  verify_mirror(record, env)?;
  if !command(
    env,
    &[
      "git",
      "-C",
      tree,
      "status",
      "--porcelain",
      "--untracked-files=all",
    ],
  )?
  .is_empty()
  {
    return Err("local result mirror is dirty; preserve it for inspection".into());
  }
  let head = command(env, &["git", "-C", tree, "rev-parse", "HEAD"])?;
  if head != field(record, "expected_base")? && head != commit {
    return Err("local mirror revision changed since dispatch".into());
  }
  command(env, &["git", "-C", tree, "merge", "--ff-only", commit])?;
  Ok(())
}

pub(super) fn verify_mirror(record: &Record, env: &Env) -> Result<(), String> {
  let tree = field(record, "worktree")?;
  let checkout = field(record, "checkout")?;
  let branch = field(record, "branch")?;
  let actual = command(env, &["git", "-C", tree, "symbolic-ref", "--short", "HEAD"])?;
  if actual != branch {
    return Err("local mirror branch identity changed".into());
  }
  let root = command(env, &["git", "-C", tree, "rev-parse", "--show-toplevel"])?;
  if std::fs::canonicalize(&root).ok() != std::fs::canonicalize(tree).ok() {
    return Err("local mirror worktree identity changed".into());
  }
  let listed = command(
    env,
    &[
      "git",
      "-C",
      checkout,
      "worktree",
      "list",
      "--porcelain",
      "-z",
    ],
  )?;
  let found = listed.split("\0\0").any(|entry| {
    entry
      .split('\0')
      .any(|line| line == format!("worktree {tree}"))
      && entry
        .split('\0')
        .any(|line| line == format!("branch refs/heads/{branch}"))
  });
  if !found {
    return Err("local mirror not owned by recorded checkout".into());
  }
  Ok(())
}

#[cfg(test)]
#[path = "tests/mirror_test.rs"]
mod tests;
