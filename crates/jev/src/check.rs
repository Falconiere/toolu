//! The `SessionStart` binary check (port of `plugins/jev/hooks/src/check-binary.ts`): advise how
//! the agent's command shell can run native `toolu`. The advice, its once-per-session record and
//! its hook output live in `toolu_runtime::startup`.

use toolu_runtime::cli::Outcome;
use toolu_runtime::env::Env;
use toolu_runtime::startup::native_toolu_hook_output;

use crate::session::silent;

/// One `SessionStart` context line, or nothing when plain `toolu` is native in the shell or
/// this session was already told.
pub fn check_binary(env: &Env, stdin: Option<&str>) -> Outcome {
  native_toolu_hook_output(env, stdin).map_or_else(silent, Outcome::data)
}

#[cfg(test)]
#[path = "tests/check_test.rs"]
mod tests;
