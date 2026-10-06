//! Atomic file replacement: the body goes to a fresh temp file beside the
//! target, created exclusively under an unguessable name (a planted symlink is
//! refused, never followed), then renamed over the target, so a reader sees the
//! old file or the new one and never a torn one.

use std::io::Write as _;
use std::path::{Path, PathBuf};

/// A temp path beside `path`: `<path>.<pid>.<nanoseconds>.tmp`.
fn temp_beside(path: &Path) -> PathBuf {
  let since = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH);
  let mut tmp = path.as_os_str().to_owned();
  tmp.push(format!(
    ".{}.{}.tmp",
    std::process::id(),
    since.map_or(0, |since| since.as_nanos())
  ));
  PathBuf::from(tmp)
}

/// Writes `body` to `path` atomically, creating its directory; `false`, with
/// no temp file left behind, when any step fails.
pub fn write_atomic(path: &Path, body: &str) -> bool {
  let tmp = temp_beside(path);
  let parent_ready = path
    .parent()
    .is_none_or(|dir| std::fs::create_dir_all(dir).is_ok());
  let created = parent_ready
    && std::fs::OpenOptions::new()
      .write(true)
      .create_new(true)
      .open(&tmp)
      .and_then(|mut file| file.write_all(body.as_bytes()))
      .is_ok();
  if created && std::fs::rename(&tmp, path).is_ok() {
    return true;
  }
  let _gone = std::fs::remove_file(&tmp);
  false
}

#[cfg(test)]
#[path = "tests/atomic_test.rs"]
mod tests;
