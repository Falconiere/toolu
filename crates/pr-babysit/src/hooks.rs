//! pr-babysit's two `SessionStart` hooks: `check-toolu` warns when Codex runs
//! this plugin without the toolu core, and `check-binary` advises how the
//! agent's shell can run a native `toolu`.

use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Outcome;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::json::ordered::Ordered;
use toolu_runtime::startup::context::{render_hook_output, session_context};
use toolu_runtime::startup::dependencies::{NoticeStyle, codex_dependency_notice};
use toolu_runtime::startup::native_toolu_advice;

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

fn session_id(env: &Env, text: Option<&str>) -> Option<String> {
  let payload = text.and_then(|text| Ordered::parse(text).ok());
  match payload
    .as_ref()
    .and_then(|payload| payload.get("session_id"))
  {
    Some(Ordered::String(id)) if !id.is_empty() => Some(id.clone()),
    _ => env.get("TOOLU_SESSION_ID").map(str::to_owned),
  }
}

/// One context message when the agent's shell cannot find a native `toolu`.
pub fn check_binary(env: &Env, text: Option<&str>) -> Outcome {
  let id = session_id(env, text);
  let output = native_toolu_advice(env, id.as_deref())
    .and_then(|line| session_context("SessionStart", &line))
    .map(|context| render_hook_output(&context, false));
  advisory(output)
}

#[cfg(test)]
#[path = "tests/hooks_test.rs"]
mod tests;
