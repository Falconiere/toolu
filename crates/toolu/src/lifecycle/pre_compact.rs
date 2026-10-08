//! `toolu hook pre-compact` (#424). The hook has nothing to say. It still loads
//! config, so a bad envelope warns, and the caller has already read stdin.

use std::path::Path;

use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Outcome;
use toolu_runtime::config::load::load;
use toolu_runtime::env::Env;
use toolu_runtime::host::detect::detect;
use toolu_runtime::host::roots::Roots;

use crate::lifecycle::diagnostics::diagnostics;

/// What the native pre-compact hook reads. The caller has already consumed stdin.
pub struct PreCompactInput<'a> {
  /// The hook's environment.
  pub env: &'a Env,
  /// The directory the hook was started in.
  pub cwd: &'a Path,
}

/// Load config and print nothing, whether or not `hooks.pre-compact` is on.
pub fn pre_compact(input: &PreCompactInput<'_>) -> Outcome {
  let detected = detect(input.env, None);
  let roots = Roots::new(input.env.clone(), Some(detected.host));
  let config = load(&roots, Some(input.cwd));
  Outcome {
    exit: Exit::Success,
    stdout: None,
    stderr: diagnostics(detected.warning, &config, Vec::new()),
  }
}

#[cfg(test)]
#[path = "tests/pre_compact_test.rs"]
mod tests;
