//! The project a command runs for: the directory it was started in, the git
//! worktree that holds it, and the git common dir when it lies outside the
//! worktree (a linked worktree's `.git` file points into the main checkout).
//! `CLAUDE_PROJECT_DIR` and friends never decide this; only where the user
//! stands does.

use std::path::{Path, PathBuf};

use toolu_runtime::env::Env;
use toolu_runtime::git::{Discovery, discover};
use toolu_runtime::process::commands::git_toplevel;
use toolu_runtime::process::{Spec, run};

/// Where a command runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Project {
  /// The directory the command starts in, canonical.
  pub(crate) cwd: PathBuf,
  /// The git worktree root holding `cwd`, when there is one.
  pub(crate) root: Option<PathBuf>,
  /// The git common dir when it lies outside `root`, as for a linked worktree.
  pub(crate) common: Option<PathBuf>,
}

/// The project for `cwd`, asking git only when the `.git` walk cannot answer.
pub(crate) fn detect(env: &Env, cwd: &Path) -> Project {
  let cwd = canonical(cwd);
  let (root, common) = match discover(env, &cwd) {
    Discovery::Repo(repo) => (repo.toplevel, Some(repo.common_dir)),
    Discovery::NotFound => (None, None),
    Discovery::AskGit => (git_toplevel(env, &cwd), ask_common(env, &cwd)),
  };
  let root = root.map(|root| canonical(&root));
  let common = common.map(|common| canonical(&common)).filter(|common| {
    root
      .as_deref()
      .is_some_and(|root| !common.starts_with(root))
  });
  Project { cwd, root, common }
}

impl Project {
  /// `cwd` relative to the worktree root; empty at the root itself.
  pub(crate) fn rel(&self) -> Option<PathBuf> {
    let root = self.root.as_deref()?;
    self.cwd.strip_prefix(root).ok().map(Path::to_path_buf)
  }

  /// The directory a VM must mount for this project: the worktree, or outside
  /// git the directory itself.
  pub(crate) fn primary(&self) -> PathBuf {
    self.root.clone().unwrap_or_else(|| self.cwd.clone())
  }

  /// What a VM must mount for this project: [`Project::primary`] and the git
  /// common dir when it lies elsewhere.
  pub(crate) fn mounts(&self) -> Vec<PathBuf> {
    let mut mounts = vec![self.primary()];
    mounts.extend(self.common.clone());
    mounts
  }
}

/// `path` with symlinks resolved, or as given when it does not exist.
pub(crate) fn canonical(path: &Path) -> PathBuf {
  std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// `git rev-parse --git-common-dir`, absolute.
fn ask_common(env: &Env, cwd: &Path) -> Option<PathBuf> {
  let mut spec = Spec::new([
    "git",
    "rev-parse",
    "--path-format=absolute",
    "--git-common-dir",
  ]);
  spec.cwd = Some(cwd.to_path_buf());
  spec.env = Some(env.clone());
  let output = run(&spec).ok().filter(|output| output.exit_code == 0)?;
  let dir = output.stdout.trim();
  (!dir.is_empty()).then(|| PathBuf::from(dir))
}

/// A stable directory name for `root`: its sanitised basename and an FNV-1a hash
/// of the whole path, so two checkouts named alike never share a clone.
pub(crate) fn slug(root: &Path) -> String {
  let base: String = root
    .file_name()
    .map(|name| name.to_string_lossy().to_lowercase())
    .unwrap_or_default()
    .chars()
    .map(|char| {
      if char.is_ascii_alphanumeric() {
        char
      } else {
        '-'
      }
    })
    .take(40)
    .collect();
  let base = base.trim_matches('-');
  let base = if base.is_empty() { "project" } else { base };
  format!(
    "{base}-{:08x}",
    fnv1a(root.as_os_str().as_encoded_bytes()) >> 32
  )
}

/// FNV-1a, 64 bits: stable across Rust releases, unlike `DefaultHasher`.
fn fnv1a(bytes: &[u8]) -> u64 {
  const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
  const PRIME: u64 = 0x0000_0100_0000_01b3;
  bytes.iter().fold(OFFSET, |hash, byte| {
    (hash ^ u64::from(*byte)).wrapping_mul(PRIME)
  })
}

#[cfg(test)]
#[path = "tests/project_test.rs"]
mod tests;
