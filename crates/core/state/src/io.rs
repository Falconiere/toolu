//! Atomic replacement of a state file (`writeAtomic` in `state-io.ts`): the
//! body goes to a fresh 0600 temp file beside the target, is synced, and is
//! renamed over it, so a reader sees the old document or the new one.

use std::io::Write as _;
use std::path::Path;

/// Writes `body` to `path` atomically; `false`, with no temp file left, when
/// any step fails (a missing directory included: none is created).
pub fn write_atomic(path: &Path, body: &str) -> bool {
  let (Some(dir), Some(name)) = (path.parent(), path.file_name()) else {
    return false;
  };
  let dir = if dir.as_os_str().is_empty() {
    Path::new(".")
  } else {
    dir
  };
  let mut prefix = name.to_os_string();
  prefix.push(".");
  let Ok(mut tmp) = tempfile::Builder::new()
    .prefix(&prefix)
    .suffix(".tmp")
    .tempfile_in(dir)
  else {
    return false;
  };
  let written = tmp.write_all(body.as_bytes()).is_ok() && tmp.as_file().sync_all().is_ok();
  if !written || tmp.persist(path).is_err() {
    return false;
  }
  // The rename is durable once the directory is synced; a failed sync loses nothing written.
  let _synced = std::fs::File::open(dir).and_then(|dir| dir.sync_all());
  true
}

#[cfg(test)]
#[path = "tests/io_test.rs"]
mod tests;
