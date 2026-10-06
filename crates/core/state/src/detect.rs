//! The shell-free half of `@toolu/core/detect`, the port of `detect.sh`: each probe
//! answers what the bash function did, without a shell, awk or grep.
//!
//! - `project`: toplevel, name, package manager, markers, linters and `to_relative_path`
//!   (only `detect_ts` spawns git);
//! - `tools`: bash `command -v` as a `PATH` scan, cached per `PATH` and name;
//! - `lines`: the code-line counters, over a bounded reader with byte semantics.
//!
//! The git questions of `detect-git.ts` need the shell parser and wait for #418.

pub mod lines;
pub mod project;
mod read;
pub mod tools;

use std::path::Path;

/// A regular file, symlinks followed: bash `[ -f ]`.
pub(crate) fn is_regular_file(path: &Path) -> bool {
  std::fs::metadata(path).is_ok_and(|meta| meta.is_file())
}

#[cfg(test)]
#[path = "tests/detect_test.rs"]
mod tests;
