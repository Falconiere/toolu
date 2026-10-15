//! A private scratch directory under the system temp directory: created by this call
//! alone with mode 0700, so a path someone planted beforehand is an error, not a home.

use std::os::unix::fs::DirBuilderExt as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

/// A new directory `<temp>/<prefix>-<pid>-<n>`; it must not exist yet.
pub(crate) fn create(prefix: &str) -> Result<PathBuf, String> {
  let tick = NEXT.fetch_add(1, Ordering::Relaxed);
  let name = format!("{prefix}-{}-{tick}", std::process::id());
  make(&std::env::temp_dir().join(name))
}

/// Create `path` alone, mode 0700, and fail when anything is already there.
pub(crate) fn make(path: &Path) -> Result<PathBuf, String> {
  std::fs::DirBuilder::new()
    .mode(0o700)
    .create(path)
    .map_err(|err| format!("cannot create {}: {err}", path.display()))?;
  Ok(path.to_path_buf())
}

#[cfg(test)]
#[path = "tests/scratch_test.rs"]
mod tests;
