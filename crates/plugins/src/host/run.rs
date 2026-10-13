//! Host commands: argv straight to the program (never a shell), in its own
//! process group with a deadline. While one runs, Ctrl-C kills its group and
//! then `toolu` by the same signal (`toolu_runtime::process::guard`).

use std::time::Duration;

use toolu_runtime::env::Env;
use toolu_runtime::process::guard::GroupGuard;
use toolu_runtime::process::{RunError, Spec, Wait, run_with};

/// How long one host command may run: a plugin install downloads.
pub(crate) const HOST_TIMEOUT: Duration = Duration::from_secs(600);

/// The bytes of output kept per command: a `plugin list --json` of every marketplace.
const MAX_OUTPUT: usize = 16 * 1_048_576;

/// What a host command left.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Ran {
  /// Its exit status; 127 when it could not start, as a shell reports it.
  pub(crate) code: i32,
  /// Its stdout.
  pub(crate) stdout: String,
  /// Its stderr, or why it could not run or finish.
  pub(crate) stderr: String,
}

impl Ran {
  /// Whether it exited 0.
  pub(crate) fn ok(&self) -> bool {
    self.code == 0
  }
}

/// Runs `argv` with `env` within `timeout`.
pub(crate) fn run(argv: &[String], env: &Env, timeout: Duration) -> Ran {
  let mut spec = Spec::new(argv.iter().cloned());
  spec.env = Some(env.clone());
  spec.timeout = timeout;
  spec.wait = Wait::Streams;
  spec.max_output_bytes = MAX_OUTPUT;
  let guard = GroupGuard::install().ok();
  let mut arm = |pid: u32| {
    if let Some(guard) = &guard {
      guard.arm(pid);
    }
    Ok(())
  };
  match run_with(&spec, &mut arm) {
    Ok(output) if output.timed_out => Ran {
      code: 124,
      stdout: output.stdout,
      stderr: format!("timed out after {} s", timeout.as_secs()),
    },
    Ok(output) => Ran {
      code: output.exit_code,
      stdout: output.stdout,
      stderr: output.stderr,
    },
    Err(err) => Ran {
      code: 127,
      stdout: String::new(),
      stderr: describe(&err),
    },
  }
}

fn describe(err: &RunError) -> String {
  match err {
    RunError::Spawn(text)
    | RunError::Wait(text)
    | RunError::Stdin(text)
    | RunError::Callback(text) => text.clone(),
    RunError::EmptyArgv => "no command to run".to_owned(),
    RunError::ZeroTimeout | RunError::TimeoutTooLong => "unusable deadline".to_owned(),
  }
}

/// The first non-blank line of `text`, trimmed, or `fallback`.
pub(crate) fn first_line(text: &str, fallback: &str) -> String {
  text
    .lines()
    .map(str::trim)
    .find(|line| !line.is_empty())
    .unwrap_or(fallback)
    .to_owned()
}

#[cfg(test)]
#[path = "tests/run_test.rs"]
mod tests;
