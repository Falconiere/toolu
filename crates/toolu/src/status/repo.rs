//! Repository facts from `git status --porcelain=v1 --branch`.

use std::path::Path;
use std::time::Duration;

use toolu_runtime::env::Env;
use toolu_runtime::process::{Spec, run};
use toolu_state::git::{current_branch, toplevel};

/// Ahead, behind and the working tree. Empty when `dir` is not a repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Repo {
  /// The git toplevel, or empty.
  pub root: String,
  /// The current branch, `"HEAD"` when detached, or empty.
  pub branch: String,
  /// Commits ahead of upstream.
  pub ahead: u64,
  /// Commits behind upstream.
  pub behind: u64,
  /// Staged paths.
  pub staged: u64,
  /// Unstaged paths.
  pub unstaged: u64,
  /// Untracked paths.
  pub untracked: u64,
}

/// How `git status --porcelain=v1 --branch` counts one repository.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct Counts {
  /// Commits ahead of upstream.
  pub ahead: u64,
  /// Commits behind upstream.
  pub behind: u64,
  /// Staged paths.
  pub staged: u64,
  /// Unstaged paths.
  pub unstaged: u64,
  /// Untracked paths.
  pub untracked: u64,
}

/// Branch and working tree of `dir`, or empty fields outside a repository.
pub(super) fn inspect(env: &Env, dir: &Path) -> Repo {
  let Some(root) = toplevel(env, dir) else {
    return Repo::empty();
  };
  let branch = current_branch(env, &root);
  let counts = porcelain(env, &root).map_or(Counts::default(), |text| counts(&text));
  Repo {
    root: root.display().to_string(),
    branch,
    ahead: counts.ahead,
    behind: counts.behind,
    staged: counts.staged,
    unstaged: counts.unstaged,
    untracked: counts.untracked,
  }
}

impl Repo {
  fn empty() -> Repo {
    Repo {
      root: String::new(),
      branch: String::new(),
      ahead: 0,
      behind: 0,
      staged: 0,
      unstaged: 0,
      untracked: 0,
    }
  }
}

/// Counts from porcelain text. A missing ahead or behind side stays 0.
pub(super) fn counts(text: &str) -> Counts {
  let mut counts = Counts::default();
  for line in text.lines() {
    if let Some(header) = line.strip_prefix("## ") {
      header_counts(&mut counts, header);
    } else {
      row_counts(&mut counts, line);
    }
  }
  counts
}

fn header_counts(counts: &mut Counts, header: &str) {
  let Some((_, bracket)) = header.split_once('[') else {
    return;
  };
  for part in bracket.trim_end_matches(']').split(',') {
    let part = part.trim();
    if let Some(ahead) = number(part, "ahead ") {
      counts.ahead = ahead;
    }
    if let Some(behind) = number(part, "behind ") {
      counts.behind = behind;
    }
  }
}

fn number(part: &str, prefix: &str) -> Option<u64> {
  part.strip_prefix(prefix)?.parse().ok()
}

fn row_counts(counts: &mut Counts, line: &str) {
  let mut chars = line.chars();
  let (Some(index), Some(work)) = (chars.next(), chars.next()) else {
    return;
  };
  if index == '?' && work == '?' {
    counts.untracked += 1;
    return;
  }
  if index != ' ' && index != '?' {
    counts.staged += 1;
  }
  if work != ' ' && work != '?' {
    counts.unstaged += 1;
  }
}

fn porcelain(env: &Env, root: &Path) -> Option<String> {
  let mut spec = Spec::new(["git", "status", "--porcelain=v1", "--branch"]);
  spec.cwd = Some(root.to_path_buf());
  spec.env = Some(env.clone());
  spec.timeout = Duration::from_secs(5);
  let output = run(&spec).ok()?;
  (output.exit_code == 0).then_some(output.stdout)
}

#[cfg(test)]
#[path = "tests/repo_test.rs"]
mod tests;
