//! A launch failure, the record fields it leaves (`failureDetails`), and the
//! checked command runner whose failures carry the native exit status.

use std::path::Path;
use std::time::Duration;

use serde_json::{Map, Value, json};
use toolu_runtime::env::Env;
use toolu_runtime::process::{Spec, run};

use crate::herdr_calls::CallError;
use crate::hosts::host_limited;

/// herdr's code for an `agent start` on a pane whose shell is not ready.
pub(crate) const PANE_BUSY: &str = "agent_pane_busy";

/// Why a launch stopped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Failure {
  /// What failed.
  pub message: String,
  /// The failing command's exit status, when one ran.
  pub exit_code: Option<i32>,
  /// herdr's error code.
  pub code: Option<String>,
  /// The effect may have happened.
  pub uncertain: bool,
}

impl Failure {
  /// A refusal: nothing ran, so nothing is uncertain.
  pub(crate) fn refusal(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
      exit_code: None,
      code: None,
      uncertain: false,
    }
  }

  /// An internal or I/O error whose effect is unknown.
  pub(crate) fn unknown(message: impl Into<String>) -> Self {
    Self {
      uncertain: true,
      ..Self::refusal(message)
    }
  }

  /// herdr refused `agent start` because the pane's shell is not at its
  /// prompt yet: a refusal that proves no agent started.
  pub(crate) fn not_started(&self) -> bool {
    self.code.as_deref() == Some(PANE_BUSY)
  }

  /// `lifecycle_error`, `lifecycle_outcome` and `native_error` for the record.
  pub(crate) fn details(&self) -> Map<String, Value> {
    let outcome = if host_limited(&self.message) {
      "provider-limited"
    } else if self.not_started() {
      "not-started"
    } else {
      "uncertain"
    };
    let mut map = Map::new();
    map.insert("lifecycle_error".to_owned(), json!(self.message));
    map.insert("lifecycle_outcome".to_owned(), json!(outcome));
    map.insert(
      "native_error".to_owned(),
      json!({"exitCode": self.exit_code, "code": self.code, "uncertain": self.uncertain}),
    );
    map
  }

  /// The stderr document of a failed `toolu epic launch`.
  pub(crate) fn report(&self, issue: &str) -> Value {
    json!({"issue": issue, "error": self.message, "native_exit_code": self.exit_code})
  }
}

impl From<CallError> for Failure {
  fn from(err: CallError) -> Self {
    Self {
      message: err.message,
      exit_code: err.exit_code,
      code: err.code,
      uncertain: err.uncertain,
    }
  }
}

impl From<String> for Failure {
  fn from(message: String) -> Self {
    Self::unknown(message)
  }
}

/// Run `argv` (in `cwd` when given) and return its stdout. A non-zero exit, a
/// timeout or a missing program fails with the command line first.
///
/// # Errors
/// The command cannot start, times out, says too much, or exits non-zero.
pub(crate) fn run_checked(
  argv: &[String],
  cwd: Option<&Path>,
  env: &Env,
  timeout: Duration,
) -> Result<String, Failure> {
  let line = argv.join(" ");
  let mut spec = Spec::new(argv.iter().cloned());
  spec.cwd = cwd.map(Path::to_path_buf);
  spec.env = Some(env.clone());
  spec.timeout = timeout;
  let out =
    run(&spec).map_err(|err| Failure::unknown(format!("{line} failed (spawn): {err:?}")))?;
  if out.exit_code != 0 || out.timed_out || out.truncated {
    let detail = if out.stderr.trim().is_empty() {
      out.stdout.trim()
    } else {
      out.stderr.trim()
    };
    let timeout = if out.timed_out { ", timeout" } else { "" };
    return Err(Failure {
      message: format!("{line} failed ({}{timeout}): {detail}", out.exit_code),
      exit_code: Some(out.exit_code),
      code: None,
      uncertain: out.timed_out,
    });
  }
  Ok(out.stdout)
}

/// `words` as owned argv.
pub(crate) fn argv(words: &[&str]) -> Vec<String> {
  words.iter().map(|word| (*word).to_owned()).collect()
}

#[cfg(test)]
#[path = "tests/failure_test.rs"]
mod tests;
