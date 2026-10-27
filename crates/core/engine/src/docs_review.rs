//! Committed documentation review identity, validation, and registry rule.

mod push;
mod report;
mod rule;

use crate::gates::push_target::git_at;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use toolu_runtime::host::roots::Roots;
use toolu_state::diff_sha::diff_sha;
use toolu_state::git::{base_branch, branch_slug};

pub use rule::{RULE, active, register};

fn git(roots: &Roots, repo: &Path, args: &[&str]) -> Result<String, String> {
  git_at(repo, args, roots.env()).ok_or_else(|| format!("git {} failed", args.join(" ")))
}

fn known_branch(roots: &Roots, root: &Path, branch: &str) -> Result<(), String> {
  let known = ["refs/heads", "refs/remotes/origin"].iter().any(|prefix| {
    git(
      roots,
      root,
      &[
        "show-ref",
        "--verify",
        "--quiet",
        &format!("{prefix}/{branch}"),
      ],
    )
    .is_ok()
  });
  if !known {
    return Err(format!("unknown branch '{branch}'"));
  }
  Ok(())
}

/// Inspect the exact committed branch diff.
///
/// # Errors
/// Returns an error if Git cannot establish the repository, branch or diff.
pub fn inspect(roots: &Roots, repo: &Path, branch: Option<&str>) -> Result<Value, String> {
  let root = git(roots, repo, &["rev-parse", "--show-toplevel"])?;
  let root = PathBuf::from(root.trim());
  let current = git(roots, &root, &["branch", "--show-current"])?;
  let branch = branch.unwrap_or(current.trim());
  if branch.is_empty() || branch == "HEAD" || branch.starts_with('-') {
    return Err("a valid branch is required".into());
  }
  git(roots, &root, &["check-ref-format", "--branch", branch])?;
  known_branch(roots, &root, branch)?;
  if !current.trim().is_empty() && current.trim() != branch {
    return Err("branch does not match checked-out branch".into());
  }
  let base = roots.env().get("DOCS_SYNC_BASE").map_or_else(
    || base_branch(roots.env(), Some(&root), &root),
    str::to_owned,
  );
  if base.starts_with('-') || base.is_empty() {
    return Err("invalid base branch".into());
  }
  git(
    roots,
    &root,
    &[
      "rev-parse",
      "--verify",
      "--quiet",
      &format!("{base}^{{commit}}"),
    ],
  )?;
  let names = git(
    roots,
    &root,
    &["diff", "--name-only", "-z", &format!("{base}...HEAD")],
  )?;
  let mut files: Vec<&str> = names.split('\0').filter(|s| !s.is_empty()).collect();
  files.sort_unstable();
  let sha = diff_sha(roots.env(), &root, &base).ok_or("cannot hash diff")?;
  Ok(
    json!({"version":2,"repo":root,"branch":branch,"base_branch":base,"diff_sha":sha,"reviewed_files":files}),
  )
}

fn state_path(roots: &Roots, repo: &Path, branch: &str) -> Result<PathBuf, String> {
  let dir = if let Some(dir) = roots.env().get("DOCS_SYNC_STATE_DIR") {
    PathBuf::from(dir)
  } else {
    roots
      .project_state_dir("docs-sync", None, Some(repo))
      .map_err(|e| e.0)?
      .ok_or("missing state directory")?
  };
  Ok(dir.join(format!("{}.json", branch_slug(branch))))
}

/// Validate and atomically write a complete review of the current committed diff.
///
/// # Errors
/// Returns an error for stale or incomplete reports, dirty tracked files, or I/O failures.
pub fn write(
  roots: &Roots,
  repo: &Path,
  branch: Option<&str>,
  report: &Value,
) -> Result<PathBuf, String> {
  let identity = inspect(roots, repo, branch)?;
  let root = Path::new(
    identity
      .get("repo")
      .and_then(Value::as_str)
      .ok_or("missing repo")?,
  );
  if !git(
    roots,
    root,
    &["status", "--porcelain", "--untracked-files=no"],
  )?
  .is_empty()
  {
    return Err("tracked working tree is dirty".into());
  }
  report::validate(roots, root, &identity, report)?;
  let path = state_path(
    roots,
    root,
    identity
      .get("branch")
      .and_then(Value::as_str)
      .ok_or("missing branch")?,
  )?;
  toolu_runtime::atomic::replace_atomic(&path, &format!("{report}\n"))
    .map_err(|e| e.to_string())?;
  Ok(path)
}

/// Assess the current committed diff against its recorded review.
///
/// # Errors
/// Returns an error if the review is absent, stale or invalid.
pub fn assess(roots: &Roots, repo: &Path, branch: &str) -> Result<(), String> {
  let identity = inspect(roots, repo, Some(branch))?;
  if identity
    .get("reviewed_files")
    .and_then(Value::as_array)
    .is_some_and(Vec::is_empty)
  {
    return Ok(());
  }
  let root = Path::new(
    identity
      .get("repo")
      .and_then(Value::as_str)
      .ok_or("missing repo")?,
  );
  let path = state_path(roots, root, branch)?;
  let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
  let report = serde_json::from_str(&text).map_err(|e| format!("invalid review: {e}"))?;
  report::validate(roots, root, &identity, &report)
}

#[cfg(test)]
#[path = "tests/docs_review_test.rs"]
mod tests;
