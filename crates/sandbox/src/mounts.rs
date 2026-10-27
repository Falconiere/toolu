//! Which host directories a VM mounts. Lima refuses overlapping mounts, so a
//! directory inside another one is dropped in favour of its ancestor, and a
//! running VM covers a path when one of its mounts holds it.

use std::path::{Path, PathBuf};

/// `paths` sorted, without duplicates or a path inside another one.
pub(crate) fn plan(paths: impl IntoIterator<Item = PathBuf>) -> Vec<PathBuf> {
  let mut sorted: Vec<PathBuf> = paths.into_iter().collect();
  sorted.sort();
  let mut kept: Vec<PathBuf> = Vec::new();
  for path in sorted {
    if !covers(&kept, &path) {
      kept.push(path);
    }
  }
  kept
}

/// Whether one of `mounts` is `path` or holds it.
pub(crate) fn covers(mounts: &[PathBuf], path: &Path) -> bool {
  mounts.iter().any(|mount| path.starts_with(mount))
}

/// The paths of `required` that `mounts` does not cover.
pub(crate) fn missing(mounts: &[PathBuf], required: &[PathBuf]) -> Vec<PathBuf> {
  required
    .iter()
    .filter(|path| !covers(mounts, path))
    .cloned()
    .collect()
}

#[cfg(test)]
#[path = "tests/mounts_test.rs"]
mod tests;
