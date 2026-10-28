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
const HOST_PREFIXES: [&str; 6] = [
  "CLAUDE_", "CODEX_", "TOOLU_", "CURSOR_", "MUSE_", "OPENCODE",
];

/// What `cargo xtask` (cargo run, rustup) adds to the environment. A host never
/// passes it to a hook, and a larger environment raises a hook's measured RSS.
const TOOLCHAIN_PREFIXES: [&str; 3] = ["CARGO", "RUSTUP_", "RUSTC"];

/// The library search path cargo run sets for the binary it runs.
const TOOLCHAIN_KEYS: [&str; 2] = ["LD_LIBRARY_PATH", "DYLD_FALLBACK_LIBRARY_PATH"];

/// Whether `key` names a repository-selecting variable.
pub(crate) fn selects_repository(key: &OsStr) -> bool {
  SELECTION.iter().any(|name| key == OsStr::new(name))
}

/// Whether `key` is one a host session sets about itself.
pub(crate) fn is_host_session(key: &OsStr) -> bool {
  let bytes = key.as_encoded_bytes();
  HOST_PREFIXES
    .iter()
    .any(|prefix| bytes.starts_with(prefix.as_bytes()))
}

/// Remove every repository-selecting variable from `command`'s environment.
pub(crate) fn scrub(command: &mut Command) {
  for name in SELECTION {
    command.env_remove(name);
  }
}

/// This process's environment, byte for byte, without the host-session keys, the
/// repository-selecting ones, the toolchain's, and the `managed` keys the caller
/// sets itself.
pub(crate) fn inherited(managed: &[&str]) -> Vec<(OsString, OsString)> {
  std::env::vars_os()
    .filter(|(key, _)| {
      let bytes = key.as_encoded_bytes();
      !selects_repository(key)
        && !is_host_session(key)
        && !managed.iter().any(|name| key == OsStr::new(name))
        && !TOOLCHAIN_KEYS.iter().any(|name| key == OsStr::new(name))
        && !TOOLCHAIN_PREFIXES
          .iter()
          .any(|prefix| bytes.starts_with(prefix.as_bytes()))
    })
    .collect()
}

#[cfg(test)]
#[path = "tests/env_scrub_test.rs"]
mod tests;
