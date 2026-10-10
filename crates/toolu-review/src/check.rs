//! `SessionStart` advice when the agent's shell cannot find a native `toolu`.

use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Outcome;
use toolu_runtime::env::Env;
use toolu_runtime::json::ordered::Ordered;
use toolu_runtime::startup::context::{render_hook_output, session_context};
use toolu_runtime::startup::native_toolu_advice;

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

/// Return one context message when the binary is absent from the command shell.
pub fn check_binary(env: &Env, text: Option<&str>) -> Outcome {
  let id = session_id(env, text);
  let output = native_toolu_advice(env, id.as_deref())
    .and_then(|line| session_context("SessionStart", &line))
    .map(|context| render_hook_output(&context, false));
  Outcome {
    exit: Exit::Success,
    stdout: output.map(|line| line.trim_end_matches('\n').to_owned()),
    stderr: None,
  }
}

#[cfg(test)]
#[path = "tests/check_test.rs"]
mod tests;
