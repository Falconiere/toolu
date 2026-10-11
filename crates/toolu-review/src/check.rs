//! `SessionStart` advice when the agent's shell cannot find a native `toolu`.

use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Outcome;
use toolu_runtime::env::Env;
use toolu_runtime::startup::native_toolu_hook_output;

/// Return one context message when the binary is absent from the command shell.
pub fn check_binary(env: &Env, text: Option<&str>) -> Outcome {
  Outcome {
    exit: Exit::Success,
    stdout: native_toolu_hook_output(env, text),
    stderr: None,
  }
}

#[cfg(test)]
#[path = "tests/check_test.rs"]
mod tests;
