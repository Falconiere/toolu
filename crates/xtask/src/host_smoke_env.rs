//! The environment every `host-smoke` child runs in: a `PATH` where `bun` and `bunx`
//! are gone but every other tool of the same directories stays, no `TOOLU_BUN`, no
//! inherited repository selection from `GIT_*`, no Muse session or credential
//! variable (`MUSE_*`), and `TOOLU_BIN` set to the native binary, so only native
//! hooks can answer. `OpenCode` gets a private `XDG_CONFIG_HOME` holding its plugins.

use std::ffi::{OsStr, OsString};
use std::os::unix::fs::{DirBuilderExt as _, PermissionsExt as _, symlink};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::env_scrub::scrub;

/// The executables a child must not find.
const BUN: [&str; 2] = ["bun", "bunx"];

/// What a child process inherits.
#[derive(Debug, Clone)]
pub(crate) struct ChildEnv {
  original: OsString,
  path: OsString,
  bin: PathBuf,
  codex_home: Option<PathBuf>,
  muse_home: Option<PathBuf>,
  opencode_config: Option<PathBuf>,
}

/// What would point Muse at the user's own config and data instead of the private home.
const MUSE_XDG: [&str; 2] = ["XDG_CONFIG_HOME", "XDG_DATA_HOME"];

/// The prefix of every Muse session and credential variable (`MUSE_AUTH_PATH`,
/// `MUSE_PLUGIN_ROOT`, `MUSE_SESSION_ID`, …), which a smoke run started from inside
/// a Muse session would otherwise pass to every child.
const MUSE_PREFIX: &str = "MUSE_";

fn is_executable(path: &Path) -> bool {
  std::fs::metadata(path).is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

fn holds_bun(dir: &Path) -> bool {
  BUN.iter().any(|name| is_executable(&dir.join(name)))
}

/// Link every executable of `dir` except `bun` and `bunx` into the new directory `shim`.
fn shim_dir(dir: &Path, shim: &Path) -> Result<(), String> {
  std::fs::DirBuilder::new()
    .mode(0o700)
    .create(shim)
    .map_err(|err| format!("cannot create {}: {err}", shim.display()))?;
  let listing =
    std::fs::read_dir(dir).map_err(|err| format!("cannot list {}: {err}", dir.display()))?;
  for entry in listing {
    let entry = entry.map_err(|err| format!("cannot list {}: {err}", dir.display()))?;
    let name = entry.file_name();
    let hidden = BUN.iter().any(|bun| name == OsStr::new(bun));
    if hidden || !is_executable(&entry.path()) {
      continue;
    }
    let link = shim.join(&name);
    symlink(entry.path(), &link).map_err(|err| format!("cannot link {}: {err}", link.display()))?;
  }
  Ok(())
}

/// `path` with each directory that holds an executable `bun` or `bunx` replaced, in its
/// own slot, by a directory of links to the other tools in it: `shims/0`, `shims/1`, in
/// `PATH` order. One directory each keeps the order of every tool across the slots.
/// `shims` must not exist yet; it is created only when such a directory does.
pub(crate) fn path_without_bun(path: &OsStr, shims: &Path) -> Result<OsString, String> {
  let mut kept = Vec::new();
  let mut made = 0_usize;
  for dir in std::env::split_paths(path) {
    if !holds_bun(&dir) {
      kept.push(dir);
      continue;
    }
    if made == 0 {
      std::fs::DirBuilder::new()
        .mode(0o700)
        .create(shims)
        .map_err(|err| format!("cannot create {}: {err}", shims.display()))?;
    }
    let shim = shims.join(made.to_string());
    shim_dir(&dir, &shim)?;
    kept.push(shim);
    made += 1;
  }
  std::env::join_paths(kept).map_err(|err| format!("cannot rebuild PATH: {err}"))
}

/// The first executable called `name` in `path`.
pub(crate) fn find_tool(name: &str, path: &OsStr) -> Option<PathBuf> {
  std::env::split_paths(path)
    .map(|dir| dir.join(name))
    .find(|candidate| is_executable(candidate))
}

impl ChildEnv {
  /// The environment for children of the native `bin`, derived from `path`; the
  /// directory `shims` (see [`path_without_bun`]) is created for it.
  pub(crate) fn new(bin: &Path, path: &OsStr, shims: &Path) -> Result<ChildEnv, String> {
    Ok(ChildEnv {
      original: path.to_owned(),
      path: path_without_bun(path, shims)?,
      bin: bin.to_path_buf(),
      codex_home: None,
      muse_home: None,
      opencode_config: None,
    })
  }

  /// The same environment with a private `CODEX_HOME`.
  pub(crate) fn with_codex_home(&self, home: &Path) -> ChildEnv {
    ChildEnv {
      codex_home: Some(home.to_path_buf()),
      ..self.clone()
    }
  }

  /// The same environment with a private `HOME` for Muse, and no XDG roots.
  pub(crate) fn with_muse_home(&self, home: &Path) -> ChildEnv {
    ChildEnv {
      muse_home: Some(home.to_path_buf()),
      ..self.clone()
    }
  }

  /// The same environment with a private `XDG_CONFIG_HOME` for `OpenCode`.
  pub(crate) fn with_opencode_config(&self, config: &Path) -> ChildEnv {
    ChildEnv {
      opencode_config: Some(config.to_path_buf()),
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
    scrub(command);
    let inherited = std::env::vars_os().map(|(key, _)| key);
    let set: Vec<OsString> = command.get_envs().map(|(key, _)| key.to_owned()).collect();
    for key in inherited.chain(set) {
      if key.as_encoded_bytes().starts_with(MUSE_PREFIX.as_bytes()) {
        command.env_remove(key);
      }
    }
    if let Some(home) = &self.codex_home {
      command.env("CODEX_HOME", home);
    }
    if let Some(config) = &self.opencode_config {
      command.env("XDG_CONFIG_HOME", config);
    }
    if let Some(home) = &self.muse_home {
      command.env("HOME", home);
      for key in MUSE_XDG {
        command.env_remove(key);
      }
    }
  }
}

#[cfg(test)]
#[path = "tests/host_smoke_env_test.rs"]
mod tests;
