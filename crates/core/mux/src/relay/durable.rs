//! Fail-closed durable relay files and directory creation.

use std::io::Write as _;
use std::os::unix::fs::{DirBuilderExt as _, OpenOptionsExt as _};
use std::path::Path;

/// Create a directory and persist its ancestry.
pub(super) fn ensure_dir(path: &Path) -> Result<(), String> {
  let path = if path.as_os_str().is_empty() {
    Path::new(".")
  } else {
    path
  };
  std::fs::DirBuilder::new()
    .recursive(true)
    .mode(0o700)
    .create(path)
    .map_err(|e| e.to_string())?;
  for dir in path.ancestors().filter(|dir| !dir.as_os_str().is_empty()) {
    sync(dir)?;
  }
  if path.is_relative() && path != Path::new(".") {
    sync(Path::new("."))?;
  }
  Ok(())
}

fn sync(path: &Path) -> Result<(), String> {
  std::fs::File::open(path)
    .and_then(|file| file.sync_all())
    .map_err(|e| e.to_string())
}

/// Replace a file atomically, syncing both the file and its directory.
///
/// # Errors
/// Returns an error if creation, writing, replacement, or synchronization fails.
pub fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
  let parent = path
    .parent()
    .filter(|p| !p.as_os_str().is_empty())
    .unwrap_or(Path::new("."));
  ensure_dir(parent)?;
  let temp = parent.join(format!(".{}.tmp", toolu_state::lock::token()));
  let result = (|| {
    create_file(&temp, bytes)?;
    std::fs::rename(&temp, path).map_err(|e| e.to_string())?;
    sync(parent)
  })();
  if result.is_err() {
    let _ = std::fs::remove_file(temp);
  }
  result
}

/// Create a new immutable marker and sync it and its parent.
pub(super) fn create(path: &Path, bytes: &[u8]) -> Result<(), String> {
  let parent = path
    .parent()
    .filter(|p| !p.as_os_str().is_empty())
    .unwrap_or(Path::new("."));
  ensure_dir(parent)?;
  create_file(path, bytes)?;
  sync(parent)
}

fn create_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
  let mut file = std::fs::OpenOptions::new()
    .write(true)
    .create_new(true)
    .mode(0o600)
    .open(path)
    .map_err(|e| e.to_string())?;
  file
    .write_all(bytes)
    .and_then(|()| file.sync_all())
    .map_err(|e| e.to_string())
}

#[cfg(test)]
#[path = "tests/durable_test.rs"]
mod tests;
