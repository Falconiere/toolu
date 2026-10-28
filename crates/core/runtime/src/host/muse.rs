//! Muse config paths and sandbox readability probes (#572).

use std::io::ErrorKind;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use crate::env::Env;

/// Muse's own config dir, ignoring `TOOLU_CONFIG_DIR` (which only relocates toolu's files).
pub fn config_dir(env: &Env) -> PathBuf {
  if let Some(dir) = env.get("MUSE_CONFIG_DIR") {
    return PathBuf::from(dir);
  }
  env.get("XDG_CONFIG_HOME").map_or_else(
    || env.home().join(".config").join("muse"),
    |base| PathBuf::from(base).join("muse"),
  )
}

/// Whether `dir` cannot be listed (sandbox hide or missing execute bit).
pub fn dir_unreadable(dir: &Path) -> bool {
  match std::fs::read_dir(dir) {
    Ok(_) => mode_denied(dir, 0o500),
    Err(error) if error.kind() == ErrorKind::PermissionDenied => true,
    Err(_) => false,
  }
}

/// Whether `path` cannot be opened for read (sandbox hide or missing read bit).
pub fn file_unreadable(path: &Path) -> bool {
  match std::fs::File::open(path) {
    Ok(_) => mode_denied(path, 0o400),
    Err(error) if error.kind() == ErrorKind::PermissionDenied => true,
    Err(_) => false,
  }
}

fn mode_denied(path: &Path, need: u32) -> bool {
  std::fs::metadata(path).is_ok_and(|meta| meta.permissions().mode() & need == 0)
}

#[cfg(test)]
#[path = "tests/muse_test.rs"]
mod tests;
