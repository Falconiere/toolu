//! Native `SessionStart`: retire the `write-state.sh` stable path older
//! versions published (#440); `toolu review write-state` replaces it.

use std::path::Path;

use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Outcome;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::startup::retire::{Retired, retire};

/// Remove toolu's old `<config root>/toolu-review/write-state.sh` link, if any.
pub fn session_start(env: &Env, _plugin_root: &Path) -> Outcome {
  let roots = Roots::new(env.clone(), None);
  let warning = match retire(
    &roots,
    "toolu-review",
    "toolu-review",
    "write-state.sh",
    None,
  ) {
    Retired::Failed(line) => Some(line),
    Retired::Absent | Retired::Removed | Retired::Kept => None,
  };
  Outcome {
    exit: Exit::Success,
    stdout: None,
    stderr: warning,
  }
}

#[cfg(test)]
#[path = "tests/session_test.rs"]
mod tests;
