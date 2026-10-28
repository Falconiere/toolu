//! What the current branch is doing: the branch and its parsed parts, the base,
//! the commits ahead of it, and the working tree's state. `log`, `context`,
//! `pr` and the `SessionStart` summary all read it.

use std::path::Path;

use toolu_runtime::env::Env;
use toolu_state::git::{base_branch, current_branch};

use crate::gitio::git;
use crate::history::{Commit, history};
use crate::settings::Settings;

/// The working tree's uncommitted changes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Dirty {
  /// Paths with staged changes.
  pub(crate) staged: u64,
  /// Tracked paths with unstaged changes.
  pub(crate) unstaged: u64,
  /// Untracked paths.
  pub(crate) untracked: u64,
}

/// The current branch against its base.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Context {
  /// The branch, `HEAD` when detached.
  pub(crate) branch: String,
  /// The base branch.
  pub(crate) base: String,
  /// The range read: `<base>..HEAD` on a feature branch, else `HEAD`.
  pub(crate) range: String,
  /// The branch is the base branch.
  pub(crate) on_base: bool,
  /// The base branch exists locally or on origin (always, on the base branch).
  pub(crate) base_found: bool,
  /// The newest commits of `range`, newest first, at most the limit.
  pub(crate) commits: Vec<Commit>,
  /// How many commits `range` holds, whatever the limit.
  pub(crate) total: usize,
  /// The working tree.
  pub(crate) dirty: Dirty,
}

fn exists(env: &Env, dir: &Path, reference: &str) -> bool {
  git(
    env,
    dir,
    &["rev-parse", "--verify", "--quiet", reference],
    &[],
  )
  .is_ok()
}

/// HEAD is on the base branch.
pub(crate) fn on_base_branch(env: &Env, dir: &Path) -> bool {
  current_branch(env, dir) == base_branch(env, None, dir)
}

/// The local base branch, else origin's.
fn base_ref(env: &Env, dir: &Path, base: &str) -> Option<String> {
  [
    format!("refs/heads/{base}"),
    format!("refs/remotes/origin/{base}"),
  ]
  .into_iter()
  .find(|reference| exists(env, dir, reference))
}

/// `git status --porcelain -z`, counted.
///
/// # Errors
/// When git cannot read the working tree.
fn dirty(env: &Env, dir: &Path) -> Result<Dirty, String> {
  let args = ["status", "--porcelain=v1", "-z", "--untracked-files=normal"];
  let text = git(env, dir, &args, &[])?;
  let mut found = Dirty::default();
  let mut entries = text.split('\0').filter(|entry| !entry.is_empty());
  while let Some(entry) = entries.next() {
    let mut letters = entry.chars();
    let (index, tree) = (letters.next(), letters.next());
    if index == Some('?') {
      found.untracked += 1;
      continue;
    }
    found.staged += u64::from(index.is_some_and(|letter| letter != ' '));
    found.unstaged += u64::from(tree.is_some_and(|letter| letter != ' '));
    if matches!(index, Some('R' | 'C')) {
      entries.next();
    }
  }
  Ok(found)
}

/// The branch HEAD names: an unborn branch by its name, `HEAD` when detached,
/// empty outside a repository.
fn branch_of(env: &Env, dir: &Path) -> String {
  let branch = current_branch(env, dir);
  if branch != "HEAD" {
    return branch;
  }
  let args = ["symbolic-ref", "--quiet", "--short", "HEAD"];
  git(env, dir, &args, &[]).map_or(branch, |name| name.trim().to_owned())
}

/// The context of the repository at `dir`, reading at most `limit` commits.
///
/// # Errors
/// When `dir` is not in a git repository, or git cannot read the range or the
/// working tree.
pub(crate) fn gather(
  env: &Env,
  dir: &Path,
  settings: &Settings,
  range: Option<&str>,
  limit: usize,
) -> Result<Context, String> {
  let branch = branch_of(env, dir);
  if branch.is_empty() {
    return Err(format!("{} is not in a git repository", dir.display()));
  }
  let base = base_branch(env, None, dir);
  let reference = (branch != base)
    .then(|| base_ref(env, dir, &base))
    .flatten();
  let on_base = branch == base;
  let base_found = on_base || reference.is_some();
  let range = match (range, &reference) {
    (Some(given), _) => given.to_owned(),
    (None, Some(reference)) => format!("{reference}..HEAD"),
    (None, None) => "HEAD".to_owned(),
  };
  let (commits, total) = if exists(env, dir, "HEAD") {
    let commits = history(env, dir, &range, limit, &settings.area_roots)?;
    let counted = git(env, dir, &["rev-list", "--count", &range, "--"], &[])?;
    (commits, counted.trim().parse().unwrap_or_default())
  } else {
    (Vec::new(), 0)
  };
  Ok(Context {
    branch,
    base,
    range,
    on_base,
    base_found,
    commits,
    total,
    dirty: dirty(env, dir)?,
  })
}

#[cfg(test)]
#[path = "tests/context_test.rs"]
mod tests;
