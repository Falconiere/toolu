//! Committed documentation review identity, validation, and registry rule.

mod push;
mod report;
mod rule;

use crate::gates::docs_sync::docs_base;
use crate::gates::push_target::git_at;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use toolu_protocol::host::Host;
use toolu_runtime::host::roots::Roots;
use toolu_state::diff_sha::diff_sha;
use toolu_state::git::branch_slug;

pub use rule::{RULE, active, register};

fn git(roots: &Roots, repo: &Path, args: &[&str]) -> Result<String, String> {
  git_at(repo, args, roots.env()).ok_or_else(|| format!("git {} failed", args.join(" ")))
}

fn known_branch(roots: &Roots, root: &Path, branch: &str) -> Result<(), String> {
  // Only a local branch: the session sweeper drops state of branches that are not.
  let reference = format!("refs/heads/{branch}");
  if git(
    roots,
    root,
    &["show-ref", "--verify", "--quiet", &reference],
  )
  .is_err()
  {
    return Err(format!("unknown local branch '{branch}'"));
  }
  Ok(())
}

/// The agent hosts a documentation worker can run on.
pub fn worker_host(name: &str) -> Option<Host> {
  Host::parse(name).filter(|host| *host != Host::Hermes)
}

/// The base a review diffs against: `DOCS_SYNC_BASE`, else the branch
/// `origin/HEAD` names (`main` when unset), then `master` and `main`, each
/// locally or on `origin`.
fn base(roots: &Roots, root: &Path) -> Result<String, String> {
  let name = docs_base(roots.env(), root);
  let candidates: Vec<String> = if roots.env().get("DOCS_SYNC_BASE").is_some() {
    vec![name.clone()]
  } else {
    [name.as_str(), "master", "main"]
      .iter()
      .flat_map(|branch| [(*branch).to_owned(), format!("origin/{branch}")])
      .collect()
  };
  candidates
    .into_iter()
    .find(|base| {
      let commit = format!("{base}^{{commit}}");
      !base.is_empty()
        && !base.starts_with('-')
        && git(roots, root, &["rev-parse", "--verify", "--quiet", &commit]).is_ok()
    })
    .ok_or_else(|| {
      format!(
        "base branch '{name}' not found; run git remote set-head origin <default-branch>, or export DOCS_SYNC_BASE before starting the host"
      )
    })
}

/// The committed branch diff a review covers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Identity {
  repo: String,
  branch: String,
  base_branch: String,
  diff_sha: String,
  reviewed_files: Vec<String>,
}

impl Identity {
  /// The identity as `toolu docs inspect` prints it and a review repeats it.
  fn json(&self) -> Value {
    json!({
      "version": 2,
      "repo": self.repo,
      "branch": self.branch,
      "base_branch": self.base_branch,
      "diff_sha": self.diff_sha,
      "reviewed_files": self.reviewed_files,
    })
  }

  fn root(&self) -> &Path {
    Path::new(&self.repo)
  }
}

/// Inspect the exact committed branch diff, as a review must repeat it.
///
/// # Errors
/// Returns an error if Git cannot establish the repository, branch or diff.
pub fn inspect(roots: &Roots, repo: &Path, branch: Option<&str>) -> Result<Value, String> {
  identify(roots, repo, branch).map(|identity| identity.json())
}

fn identify(roots: &Roots, repo: &Path, branch: Option<&str>) -> Result<Identity, String> {
  let top = git(roots, repo, &["rev-parse", "--show-toplevel"])?;
  let root = Path::new(top.trim());
  let current = git(roots, root, &["branch", "--show-current"])?;
  let branch = branch.unwrap_or(current.trim());
  if branch.is_empty() || branch == "HEAD" || branch.starts_with('-') {
    return Err("a valid branch is required".into());
  }
  git(roots, root, &["check-ref-format", "--branch", branch])?;
  known_branch(roots, root, branch)?;
  if !current.trim().is_empty() && current.trim() != branch {
    return Err("branch does not match checked-out branch".into());
  }
  let base = base(roots, root)?;
  let names = git(
    roots,
    root,
    &["diff", "--name-only", "-z", &format!("{base}...HEAD")],
  )?;
  let mut reviewed_files: Vec<String> = names
    .split('\0')
    .filter(|s| !s.is_empty())
    .map(str::to_owned)
    .collect();
  reviewed_files.sort_unstable();
  let diff_sha = diff_sha(roots.env(), root, &base).ok_or("cannot hash diff")?;
  Ok(Identity {
    repo: root.to_string_lossy().into_owned(),
    branch: branch.to_owned(),
    base_branch: base,
    diff_sha,
    reviewed_files,
  })
}

/// The review file of `branch` in the state directory of `roots`' host.
fn review_file(roots: &Roots, root: &Path, branch: &str) -> Result<PathBuf, String> {
  let dir = roots
    .project_state_dir("docs-review", None, Some(root))
    .map_err(|e| e.0)?
    .ok_or("missing state directory")?;
  Ok(dir.join(format!("{}.json", branch_slug(branch))))
}

/// Every host's review file of `branch`: a review records the diff, not the
/// host that wrote it, and agent shells do not always identify their host.
fn review_files(roots: &Roots, root: &Path, branch: &str) -> Vec<PathBuf> {
  let mut files = Vec::new();
  for host in Host::ALL {
    let roots = Roots::new(roots.env().clone(), Some(host));
    if let Ok(file) = review_file(&roots, root, branch)
      && !files.contains(&file)
    {
      files.push(file);
    }
  }
  files
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
  let identity = identify(roots, repo, branch)?;
  let root = identity.root();
  let status = git(
    roots,
    root,
    &["status", "--porcelain", "--untracked-files=no"],
  )?;
  if !status.is_empty() {
    return Err("tracked working tree is dirty".into());
  }
  report::validate(roots, &identity, report)?;
  let path = review_file(roots, root, &identity.branch)?;
  toolu_runtime::atomic::replace_atomic(&path, &format!("{report}\n"))
    .map_err(|e| e.to_string())?;
  Ok(path)
}

/// Assess the current committed diff against its recorded review, from any
/// host's state directory.
///
/// # Errors
/// Returns an error if the review is absent, stale or invalid.
pub fn assess(roots: &Roots, repo: &Path, branch: &str) -> Result<(), String> {
  let identity = identify(roots, repo, Some(branch))?;
  if identity.reviewed_files.is_empty() {
    return Ok(());
  }
  let sync = roots
    .invocation("documentation-gate", "sync")
    .unwrap_or_default();
  let mut outcome = Err(format!(
    "no review is recorded for this diff; run {sync} and record it with toolu docs write-state"
  ));
  for file in review_files(roots, identity.root(), branch) {
    let checked = match std::fs::read_to_string(&file) {
      Ok(text) => serde_json::from_str(&text)
        .map_err(|e| format!("invalid review: {e}"))
        .and_then(|report| report::validate(roots, &identity, &report)),
      Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
      Err(e) => Err(format!("cannot read {}: {e}", file.display())),
    };
    if checked.is_ok() {
      return checked;
    }
    outcome = checked;
  }
  outcome
}

#[cfg(test)]
#[path = "tests/docs_review_test.rs"]
mod tests;
