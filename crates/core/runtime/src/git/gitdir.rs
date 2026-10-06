//! What git accepts as a repository directory (`is_git_directory`,
//! `read_gitfile_gently` and `get_common_dir` in git's `setup.c`): a git dir
//! holds a valid `HEAD`, and its common dir holds `objects/` and `refs/`; a
//! `.git` file names one with `gitdir: <path>`.

use std::path::{Path, PathBuf};

/// The common dir of `git_dir`: its `commondir` file resolved against it
/// (a linked worktree's admin dir), else `git_dir` itself.
pub(crate) fn common_dir_of(git_dir: &Path) -> PathBuf {
  let Ok(text) = std::fs::read_to_string(git_dir.join("commondir")) else {
    return git_dir.to_path_buf();
  };
  let named = git_dir.join(text.trim_end_matches(['\n', '\r']));
  std::fs::canonicalize(&named).unwrap_or(named)
}

/// Whether `path` is a git directory: a valid `HEAD`, and `objects/` and
/// `refs/` under its common dir.
pub(crate) fn is_git_dir(path: &Path) -> bool {
  if !valid_head(&path.join("HEAD")) {
    return false;
  }
  let common = common_dir_of(path);
  common.join("objects").is_dir() && common.join("refs").is_dir()
}

/// `validate_headref`: a symlink into `refs/`, a `ref: refs/…` line, or an object id.
fn valid_head(head: &Path) -> bool {
  let Ok(meta) = std::fs::symlink_metadata(head) else {
    return false;
  };
  if meta.file_type().is_symlink() {
    return std::fs::read_link(head).is_ok_and(|target| target.starts_with("refs"));
  }
  let Ok(text) = std::fs::read_to_string(head) else {
    return false;
  };
  if let Some(rest) = text.strip_prefix("ref:") {
    return rest.trim_start().starts_with("refs/");
  }
  is_object_id(text.trim_end())
}

/// A SHA-1 or SHA-256 object id in hex.
pub(crate) fn is_object_id(text: &str) -> bool {
  matches!(text.len(), 40 | 64) && text.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// The git dir a `.git` file names, made absolute and real, or `None` when
/// the file is not `gitdir: <path>` naming a git directory (git dies then).
pub(crate) fn read_gitfile(file: &Path) -> Option<PathBuf> {
  let text = std::fs::read_to_string(file).ok()?;
  let named = text
    .strip_prefix("gitdir: ")?
    .trim_end_matches(['\n', '\r']);
  if named.is_empty() {
    return None;
  }
  let base = file.parent()?;
  let dir = base.join(named);
  if !is_git_dir(&dir) {
    return None;
  }
  std::fs::canonicalize(&dir).ok()
}

#[cfg(test)]
#[path = "tests/gitdir_test.rs"]
mod tests;
