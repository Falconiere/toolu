//! When the `.git` walk cannot answer as git would, and git must be asked:
//! variables that redirect discovery, repositories git may refuse for their
//! owner (`safe.directory`), and config that moves or removes the worktree.

use std::os::unix::fs::MetadataExt as _;
use std::path::Path;

use crate::env::Env;

/// Variables that change where git looks or what it reads.
const REDIRECTING: [&str; 9] = [
  "GIT_DIR",
  "GIT_WORK_TREE",
  "GIT_COMMON_DIR",
  "GIT_OBJECT_DIRECTORY",
  "GIT_CEILING_DIRECTORIES",
  "GIT_DISCOVERY_ACROSS_FILESYSTEM",
  "GIT_CONFIG_PARAMETERS",
  "GIT_CONFIG_COUNT",
  "GIT_CONFIG",
];

/// Whether `env` redirects discovery, or runs as root under `sudo`, where git
/// judges ownership against `SUDO_UID`.
pub(crate) fn env_defers(env: &Env) -> bool {
  let sudo = nix::unistd::geteuid().is_root() && env.get("SUDO_UID").is_some();
  sudo || REDIRECTING.iter().any(|key| env.get(key).is_some())
}

/// Whether any of `paths` (the worktree, the `.git` file, the git dir) is
/// owned by someone other than the effective user, so git's
/// `safe.directory` rule decides.
pub(crate) fn foreign_owner(paths: &[&Path]) -> bool {
  let euid = nix::unistd::geteuid().as_raw();
  paths
    .iter()
    .any(|path| !std::fs::symlink_metadata(path).is_ok_and(|meta| meta.uid() == euid))
}

/// Whether the repository config moves or removes the worktree: a
/// `config.worktree` file, an include, `core.worktree`, or, when a `.git` gave
/// the repository a worktree, `core.bare` true.
pub(crate) fn config_defers(git_dir: &Path, common_dir: &Path, has_worktree: bool) -> bool {
  if git_dir.join("config.worktree").exists() || common_dir.join("config.worktree").exists() {
    return true;
  }
  std::fs::read_to_string(common_dir.join("config"))
    .is_ok_and(|text| moves_worktree(&text, has_worktree))
}

/// The `[core]` keys of git config `text` that move the worktree, or an include.
fn moves_worktree(text: &str, has_worktree: bool) -> bool {
  let mut section = String::new();
  for raw in text.lines() {
    let mut line = raw.trim_start();
    if let Some(rest) = line.strip_prefix('[') {
      let Some((header, after)) = rest.split_once(']') else {
        continue;
      };
      section = header.trim().to_ascii_lowercase();
      if section.starts_with("include") {
        return true;
      }
      line = after.trim_start();
    }
    if section == "core" && core_key_moves(line, has_worktree) {
      return true;
    }
  }
  false
}

fn core_key_moves(line: &str, has_worktree: bool) -> bool {
  let (key, value) = match line.split_once('=') {
    Some((key, value)) => (key, Some(value)),
    None => (line, None),
  };
  match key.trim().to_ascii_lowercase().as_str() {
    "worktree" => true,
    "bare" => has_worktree && value.is_none_or(truthy),
    _ => false,
  }
}

/// A git boolean value that is true; an empty value is false.
fn truthy(value: &str) -> bool {
  let value = value.split(['#', ';']).next().unwrap_or_default();
  let value = value.trim().trim_matches('"').to_ascii_lowercase();
  !matches!(value.as_str(), "" | "false" | "no" | "off" | "0")
}

#[cfg(test)]
#[path = "tests/defer_test.rs"]
mod tests;
