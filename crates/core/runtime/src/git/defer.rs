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
  // Git treats an empty `GIT_DIR` or `GIT_WORK_TREE` as set, so presence is what counts.
  sudo || env.vars().any(|(key, _)| REDIRECTING.contains(&key))
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

/// The worktree settings of a repository's `[core]` config.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct Core {
  /// `core.bare` is true: no worktree.
  pub(crate) bare: bool,
  /// `core.worktree` as written: the worktree, relative to the git dir.
  pub(crate) worktree: Option<String>,
}

/// The `[core]` worktree settings, or `None` when only git can read them: a
/// `config.worktree` file, an include, a malformed header, a quoted or escaped
/// value, a valueless `worktree`, a repository format above 1, or a config
/// that cannot be read as UTF-8.
pub(crate) fn core_config(git_dir: &Path, common_dir: &Path) -> Option<Core> {
  if git_dir.join("config.worktree").exists() || common_dir.join("config.worktree").exists() {
    return None;
  }
  match std::fs::read_to_string(common_dir.join("config")) {
    Ok(text) => parse_core(&text),
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => Some(Core::default()),
    // Unreadable or not UTF-8: only git can read it.
    Err(_) => None,
  }
}

fn parse_core(text: &str) -> Option<Core> {
  let mut core = Core::default();
  let mut section = String::new();
  for raw in text.lines() {
    let mut line = raw.trim_start();
    if let Some(rest) = line.strip_prefix('[') {
      let (header, after) = rest.split_once(']')?;
      section = header.trim().to_ascii_lowercase();
      if section.starts_with("include") {
        return None;
      }
      line = after.trim_start();
    }
    if section == "core" {
      core_key(&mut core, line)?;
    }
  }
  Some(core)
}

/// Applies one `[core]` line to `core`; `None` when its value needs git.
fn core_key(core: &mut Core, line: &str) -> Option<()> {
  let (key, value) = match line.split_once('=') {
    Some((key, value)) => (key, Some(value)),
    None => (line, None),
  };
  let key = key.trim().to_ascii_lowercase();
  if !matches!(
    key.as_str(),
    "bare" | "worktree" | "repositoryformatversion"
  ) {
    return Some(());
  }
  let value = match value {
    Some(raw) => Some(plain(raw)?),
    None => None,
  };
  match (key.as_str(), value) {
    ("bare", value) => core.bare = value.as_deref().is_none_or(truthy),
    ("worktree", Some(value)) if !value.is_empty() => core.worktree = Some(value),
    ("repositoryformatversion", Some(version)) if version.parse::<u32>().is_ok_and(|v| v <= 1) => {}
    _ => return None,
  }
  Some(())
}

/// A config value with its comment and blanks removed; `None` when quoted or escaped.
fn plain(value: &str) -> Option<String> {
  let value = value.split(['#', ';']).next().unwrap_or("").trim();
  (!value.contains(['"', '\\'])).then(|| value.to_owned())
}

/// A git boolean value that is true; an empty value is false.
fn truthy(value: &str) -> bool {
  !matches!(
    value.to_ascii_lowercase().as_str(),
    "" | "false" | "no" | "off" | "0"
  )
}

#[cfg(test)]
#[path = "tests/defer_test.rs"]
mod tests;
