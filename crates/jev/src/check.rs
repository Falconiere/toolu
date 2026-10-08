//! The `SessionStart` binary check (port of `plugins/jev/hooks/src/check-binary.ts`): advise how
//! the agent's command shell can run native `toolu`. The advice and its once-per-session
//! record live in `toolu_runtime::startup`; this only renders it as context.

use toolu_runtime::cli::Outcome;
use toolu_runtime::env::Env;
use toolu_runtime::json::ordered::Ordered;
use toolu_runtime::startup::native_toolu_advice;

use crate::session::{context_outcome, payload, silent};

/// The payload's string `session_id`, else `TOOLU_SESSION_ID`. A missing id prints every time.
fn session_id(env: &Env, stdin: Option<&str>) -> Option<String> {
  let input = payload(stdin);
  match input.as_ref().and_then(|input| input.get("session_id")) {
    Some(Ordered::String(id)) if !id.is_empty() => Some(id.clone()),
    _ => env.get("TOOLU_SESSION_ID").map(str::to_owned),
  }
}

/// One `SessionStart` context line, or nothing when plain `toolu` is native in the shell or
/// this session was already told.
pub fn check_binary(env: &Env, stdin: Option<&str>) -> Outcome {
  let id = session_id(env, stdin);
  match native_toolu_advice(env, id.as_deref()) {
    Some(line) => context_outcome("SessionStart", &line),
    None => silent(),
  }
}

#[cfg(test)]
#[path = "tests/check_test.rs"]
mod tests;
