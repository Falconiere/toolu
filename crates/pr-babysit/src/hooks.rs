//! pr-babysit's two `SessionStart` hooks: `check-toolu` warns when Codex runs
//! this plugin without the toolu core, and `check-binary` advises how the
//! agent's shell can run a native `toolu`.

use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Outcome;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::startup::dependencies::{NoticeStyle, codex_dependency_notice};
use toolu_runtime::startup::native_toolu_hook_output;

/// The core plugin pr-babysit needs.
const CORE_PLUGIN: &str = "toolu@toolu";

fn advisory(stdout: Option<String>) -> Outcome {
  Outcome {
    exit: Exit::Success,
    stdout: stdout.map(|line| line.trim_end_matches('\n').to_owned()),
    stderr: None,
  }
}

/// The Codex warning when the toolu core is not installed; silent elsewhere.
pub fn check_toolu(env: &Env) -> Outcome {
  let roots = Roots::new(env.clone(), None);
  advisory(codex_dependency_notice(
    &[CORE_PLUGIN],
    NoticeStyle::Core,
    &roots,
  ))
}

/// One context message when the agent's shell cannot find a native `toolu`.
pub fn check_binary(env: &Env, text: Option<&str>) -> Outcome {
  advisory(native_toolu_hook_output(env, text))
}

#[cfg(test)]
#[path = "tests/hooks_test.rs"]
mod tests;
