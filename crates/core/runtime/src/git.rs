//! Repository discovery from the filesystem (#415): the toplevel, git dir and
//! common dir holding a directory, found by walking up for `.git` as git's
//! `setup_git_directory_gently_1` does, so no process is spawned. Where the
//! walk cannot answer as git would (the `GIT_DIR` variables, `sudo`, another
//! owner, `core.worktree`, `core.bare`, `config.worktree`, includes), the
//! answer is [`Discovery::AskGit`].

mod defer;
mod gitdir;

use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};

use crate::env::Env;
use crate::process::commands::git_toplevel;

/// A repository found by the walk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repo {
  /// The worktree root; `None` for a bare repository or inside a git dir.
  pub toplevel: Option<PathBuf>,
  /// The git dir: `.git`, the dir a `.git` file names, or the bare repository.
  pub git_dir: PathBuf,
  /// The common dir: where `objects/`, `refs/` and the config live.
  pub common_dir: PathBuf,
}

/// What the walk found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Discovery {
  /// A repository git would find the same way.
  Repo(Repo),
  /// No repository, as git would report.
  NotFound,
  /// Only git can answer exactly; see the module docs.
  AskGit,
}

/// One directory of the walk.
enum Step {
  Found(Repo, Option<PathBuf>),
  Invalid,
  Continue,
}

/// The repository holding `cwd`, read from the filesystem.
pub fn discover(env: &Env, cwd: &Path) -> Discovery {
  if defer::env_defers(env) {
    return Discovery::AskGit;
  }
  let Ok(start) = std::fs::canonicalize(cwd) else {
    return Discovery::NotFound;
  };
  let Some(device) = device_of(&start) else {
    return Discovery::NotFound;
  };
  let mut dir = start.as_path();
  loop {
    match look(dir) {
      Step::Found(repo, gitfile) => return checked(repo, gitfile.as_deref()),
      Step::Invalid => return Discovery::NotFound,
      Step::Continue => {}
    }
    match dir.parent() {
      Some(parent) if device_of(parent) == Some(device) => dir = parent,
      _ => return Discovery::NotFound,
    }
  }
}

/// `git rev-parse --show-toplevel` from `cwd`, without spawning git unless the
/// walk defers to it.
pub fn toplevel(env: &Env, cwd: &Path) -> Option<PathBuf> {
  match discover(env, cwd) {
    Discovery::Repo(repo) => repo.toplevel,
    Discovery::NotFound => None,
    Discovery::AskGit => git_toplevel(env, cwd),
  }
}

fn device_of(path: &Path) -> Option<u64> {
  std::fs::metadata(path).ok().map(|meta| meta.dev())
}

/// `dir/.git` as a gitfile or a git dir, then `dir` itself as a git dir.
fn look(dir: &Path) -> Step {
  let dot = dir.join(".git");
  if let Ok(meta) = std::fs::metadata(&dot) {
    if meta.is_file() {
      return match gitdir::read_gitfile(&dot) {
        Some(git_dir) => Step::Found(repo(Some(dir), git_dir), Some(dot)),
        None => Step::Invalid,
      };
    }
    if gitdir::is_git_dir(&dot) {
      return Step::Found(repo(Some(dir), dot), None);
    }
  }
  if gitdir::is_git_dir(dir) {
    return Step::Found(repo(None, dir.to_path_buf()), None);
  }
  Step::Continue
}

fn repo(toplevel: Option<&Path>, git_dir: PathBuf) -> Repo {
  Repo {
    toplevel: toplevel.map(Path::to_path_buf),
    common_dir: gitdir::common_dir_of(&git_dir),
    git_dir,
  }
}

/// `repo`, unless its owner or its config leaves the answer to git.
fn checked(repo: Repo, gitfile: Option<&Path>) -> Discovery {
  let owned: Vec<&Path> = [repo.toplevel.as_deref(), gitfile]
    .into_iter()
    .flatten()
    .chain([repo.git_dir.as_path()])
    .collect();
  let worktree = repo.toplevel.is_some();
  if defer::foreign_owner(&owned) || defer::config_defers(&repo.git_dir, &repo.common_dir, worktree)
  {
    return Discovery::AskGit;
  }
  Discovery::Repo(repo)
}

#[cfg(test)]
#[path = "tests/git_test.rs"]
mod tests;
