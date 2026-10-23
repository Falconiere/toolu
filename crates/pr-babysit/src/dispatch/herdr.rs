//! Commands the dispatcher runs — git and the herdr CLI — and the herdr agent
//! moves: is it live, stop it, accept Claude's plain trust prompt.

use std::time::Duration;

use serde_json::Value;
use toolu_runtime::env::Env;
use toolu_runtime::process::{Spec, run};

use super::outcome::is_claude_trust_prompt;
use crate::error::{Failure, Result};
use crate::fixer::host::Fixer;

const TIMEOUT: Duration = Duration::from_secs(120);
const SECOND: Duration = Duration::from_secs(1);

/// A command's exit, stdout and stderr.
pub(crate) struct Ran {
  /// The exit code; 1 when it could not run.
  pub(crate) status: i32,
  /// Its stdout.
  pub(crate) output: String,
  /// Its stderr, where herdr writes the error document of a failed call.
  pub(crate) errors: String,
}

/// Runs `argv` with `env`; a command that cannot start exits 1.
pub(crate) fn command(env: &Env, argv: &[String], timeout: Duration) -> Ran {
  let mut spec = Spec::new(argv.iter().cloned());
  spec.env = Some(env.clone());
  spec.timeout = timeout;
  match run(&spec) {
    Ok(output) => Ran {
      status: output.exit_code,
      output: output.stdout,
      errors: output.stderr,
    },
    Err(error) => Ran {
      status: 1,
      output: format!("{error:?}"),
      errors: String::new(),
    },
  }
}

/// `argv` as owned strings.
pub(crate) fn argv(parts: &[&str]) -> Vec<String> {
  parts.iter().map(|part| (*part).to_owned()).collect()
}

/// A herdr error: its code and message.
pub(crate) struct HerdrError {
  /// herdr's error code, or `invalid_output`.
  pub(crate) code: String,
  /// Its message.
  pub(crate) message: String,
}

/// `herdr <args>` with a deadline: its `result`, or its error.
pub(crate) fn try_herdr(
  env: &Env,
  args: &[String],
  timeout: Duration,
) -> std::result::Result<Value, HerdrError> {
  let mut full = vec!["herdr".to_owned()];
  full.extend_from_slice(args);
  let ran = command(env, &full, timeout);
  let parsed = serde_json::from_str::<Value>(&ran.output).ok().or_else(|| {
    serde_json::from_str::<Value>(&ran.errors)
      .ok()
      .filter(|document| document.get("error").is_some())
  });
  let Some(mut value) = parsed else {
    let output = if ran.output.trim().is_empty() {
      &ran.errors
    } else {
      &ran.output
    };
    let shown: String = output.trim().chars().take(200).collect();
    return Err(HerdrError {
      code: "invalid_output".into(),
      message: format!("exit {}: {shown}", ran.status),
    });
  };
  if let Some(error) = value.get("error") {
    let field = |key: &str, default: &str| {
      error
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or(default)
        .to_owned()
    };
    return Err(HerdrError {
      code: field("code", "unknown"),
      message: field("message", ""),
    });
  }
  Ok(match value.get_mut("result").map(Value::take) {
    Some(result) => result,
    None => value,
  })
}

/// `herdr <args>`, any error a `herdr_error` failure.
///
/// # Errors
/// `herdr_error` naming the subcommand, herdr's code and message.
pub(crate) fn herdr(env: &Env, args: &[String]) -> Result<Value> {
  try_herdr(env, args, TIMEOUT).map_err(|error| {
    let verb = args.iter().take(2).cloned().collect::<Vec<_>>().join(" ");
    Failure::new(
      "herdr_error",
      format!("herdr {verb}: {}: {}", error.code, error.message),
    )
    .with("herdrCode", error.code)
  })
}

/// Whether the herdr server answers.
pub(crate) fn reachable(env: &Env) -> bool {
  try_herdr(env, &argv(&["workspace", "list"]), TIMEOUT).is_ok()
}

fn agent_live(env: &Env, name: &str) -> bool {
  let listed = try_herdr(env, &argv(&["agent", "list"]), TIMEOUT).ok();
  let agents = listed
    .as_ref()
    .and_then(|value| value.get("agents")?.as_array());
  agents.is_some_and(|agents| {
    agents
      .iter()
      .any(|agent| agent.get("name").and_then(Value::as_str) == Some(name))
  })
}

/// Asks agent `name`, a `host` fixer, to exit and waits up to 30 s; true when
/// it is gone.
pub(crate) fn stop_agent(env: &Env, name: &str, host: Option<Fixer>) -> bool {
  if !agent_live(env, name) {
    return true;
  }
  let _escaped = try_herdr(env, &argv(&["agent", "send-keys", name, "esc"]), TIMEOUT);
  let _exited = try_herdr(env, &argv(&["agent", "prompt", name, "/exit"]), TIMEOUT);
  // With live background work Muse asks "Exit anyway?" and enter confirms it.
  // An idle Muse has already left: the key lands at the pane shell or herdr
  // answers agent_not_found, and either way the liveness wait decides.
  if host == Some(Fixer::Muse) {
    let _confirmed = try_herdr(env, &argv(&["agent", "send-keys", name, "enter"]), TIMEOUT);
  }
  for _ in 0..30 {
    if !agent_live(env, name) {
      return true;
    }
    std::thread::sleep(SECOND);
  }
  false
}

/// The last 40 lines of agent `name`'s screen.
pub(crate) fn screen(env: &Env, name: &str) -> Ran {
  let read = argv(&[
    "herdr",
    "agent",
    "read",
    name,
    "--source",
    "recent-unwrapped",
    "--lines",
    "40",
  ]);
  command(env, &read, TIMEOUT)
}

/// Answers Claude's plain folder-trust prompt for `path`, then waits up to
/// 30 s for the agent to go idle; false when the screen is anything else.
pub(crate) fn accept_claude_trust(env: &Env, name: &str, path: &str) -> bool {
  let pane = screen(env, name);
  if pane.status != 0 || !is_claude_trust_prompt(&pane.output, path) {
    return false;
  }
  for key in ["down", "enter"] {
    if try_herdr(env, &argv(&["agent", "send-keys", name, key]), TIMEOUT).is_err() {
      return false;
    }
  }
  for _ in 0..30 {
    let got = try_herdr(env, &argv(&["agent", "get", name]), TIMEOUT).ok();
    let status = got
      .as_ref()
      .and_then(|value| value.pointer("/agent/agent_status")?.as_str());
    if status == Some("idle") {
      return true;
    }
    std::thread::sleep(SECOND);
  }
  false
}

#[cfg(test)]
#[path = "tests/herdr_test.rs"]
mod tests;
