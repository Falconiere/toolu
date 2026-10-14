//! The environment every `host-smoke` child runs in: a `PATH` without any directory
//! that holds a `bun` executable, no `TOOLU_BUN`, and `TOOLU_BIN` set to the native
//! binary, so only native hooks can answer.

use std::ffi::{OsStr, OsString};
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

/// What a child process inherits.
#[derive(Debug, Clone)]
pub(crate) struct ChildEnv {
  original: OsString,
  path: OsString,
  bin: PathBuf,
  codex_home: Option<PathBuf>,
}

fn is_executable(path: &Path) -> bool {
  std::fs::metadata(path).is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

/// `path` without the directories that contain an executable `bun`.
pub(crate) fn path_without_bun(path: &OsStr) -> Result<OsString, String> {
  let kept: Vec<PathBuf> = std::env::split_paths(path)
    .filter(|dir| !is_executable(&dir.join("bun")))
    .collect();
  std::env::join_paths(kept).map_err(|err| format!("cannot rebuild PATH: {err}"))
}

/// The first executable called `name` in `path`.
pub(crate) fn find_tool(name: &str, path: &OsStr) -> Option<PathBuf> {
  std::env::split_paths(path)
    .map(|dir| dir.join(name))
    .find(|candidate| is_executable(candidate))
}

impl ChildEnv {
  /// The environment for children of the native `bin`, derived from `path`.
  pub(crate) fn new(bin: &Path, path: &OsStr) -> Result<ChildEnv, String> {
    Ok(ChildEnv {
      original: path.to_owned(),
      path: path_without_bun(path)?,
      bin: bin.to_path_buf(),
      codex_home: None,
    })
  }

  /// The same environment with a private `CODEX_HOME`.
  pub(crate) fn with_codex_home(&self, home: &Path) -> ChildEnv {
    ChildEnv {
      codex_home: Some(home.to_path_buf()),
      ..self.clone()
    }
  }

  /// The absolute path of the host CLI `name`, found on the unfiltered `PATH`.
  pub(crate) fn tool(&self, name: &str) -> Result<PathBuf, String> {
    find_tool(name, &self.original).ok_or_else(|| format!("`{name}` is not on PATH"))
  }

  /// Give `command` this environment.
  pub(crate) fn apply(&self, command: &mut Command) {
    command
      .env("PATH", &self.path)
      .env("TOOLU_BIN", &self.bin)
      .env("GIT_CONFIG_NOSYSTEM", "1")
      .env_remove("TOOLU_BUN");
    if let Some(home) = &self.codex_home {
      command.env("CODEX_HOME", home);
    }
  }
}

#[cfg(test)]
#[path = "tests/host_smoke_env_test.rs"]
mod tests;
