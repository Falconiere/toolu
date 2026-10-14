//! What a measured or smoke-tested child must not inherit: the variables that point git
//! at a repository other than its working directory (a hook run from inside `git commit`
//! would otherwise build the wrong repository), and, for the benchmarks, whatever a host
//! session exports about itself.

use std::ffi::{OsStr, OsString};
use std::process::Command;

const SELECTION: [&str; 6] = [
  "GIT_DIR",
  "GIT_WORK_TREE",
  "GIT_INDEX_FILE",
  "GIT_COMMON_DIR",
  "GIT_OBJECT_DIRECTORY",
  "GIT_ALTERNATE_OBJECT_DIRECTORIES",
];

/// Environment key prefixes a host session sets.
const HOST_PREFIXES: [&str; 4] = ["CLAUDE_", "CODEX_", "TOOLU_", "CURSOR_"];

/// Whether `key` names a repository-selecting variable.
pub(crate) fn selects_repository(key: &OsStr) -> bool {
  SELECTION.iter().any(|name| key == OsStr::new(name))
}

/// Remove every repository-selecting variable from `command`'s environment.
pub(crate) fn scrub(command: &mut Command) {
  for name in SELECTION {
    command.env_remove(name);
  }
}

/// This process's environment, byte for byte, without the host-session keys, the
/// repository-selecting ones, and the `managed` keys the caller sets itself.
pub(crate) fn inherited(managed: &[&str]) -> Vec<(OsString, OsString)> {
  std::env::vars_os()
    .filter(|(key, _)| {
      let bytes = key.as_encoded_bytes();
      !selects_repository(key)
        && !managed.iter().any(|name| key == OsStr::new(name))
        && !HOST_PREFIXES
          .iter()
          .any(|prefix| bytes.starts_with(prefix.as_bytes()))
    })
    .collect()
}

#[cfg(test)]
#[path = "tests/env_scrub_test.rs"]
mod tests;
