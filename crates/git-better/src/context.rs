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
  /// The branch is the base branch, or no base ref exists.
  pub(crate) on_base: bool,
  /// The commits of `range`, newest first.
  pub(crate) commits: Vec<Commit>,
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
fn dirty(env: &Env, dir: &Path) -> Dirty {
  let args = ["status", "--porcelain=v1", "-z", "--untracked-files=normal"];
  let text = git(env, dir, &args, &[]).unwrap_or_default();
  let mut found = Dirty::default();
  let mut entries = text.split('\0').filter(|entry| entry.len() > 3);
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
  found
}

/// The context of the repository at `dir`, reading at most `limit` commits.
///
/// # Errors
/// When `dir` is not in a git repository, or git cannot read the range.
pub(crate) fn gather(
  env: &Env,
  dir: &Path,
  settings: &Settings,
  range: Option<&str>,
  limit: usize,
) -> Result<Context, String> {
  let branch = current_branch(env, dir);
  if branch.is_empty() {
    return Err(format!("{} is not in a git repository", dir.display()));
  }
  let base = base_branch(env, None, dir);
  let reference = (branch != base)
    .then(|| base_ref(env, dir, &base))
    .flatten();
  let on_base = reference.is_none();
  let range = match (range, &reference) {
    (Some(given), _) => given.to_owned(),
    (None, Some(reference)) => format!("{reference}..HEAD"),
    (None, None) => "HEAD".to_owned(),
  };
  let commits = if exists(env, dir, "HEAD") {
    history(env, dir, &range, limit, &settings.area_roots)?
  } else {
    Vec::new()
  };
  Ok(Context {
    branch,
    base,
    range,
    on_base,
    commits,
    dirty: dirty(env, dir),
  })
}

#[cfg(test)]
#[path = "tests/context_test.rs"]
mod tests;
