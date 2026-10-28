//! Running `limactl`: short questions (`list`, `--version`, `stop`, `edit`,
//! `delete`) whose output is read back, and long steps (booting, provisioning)
//! whose output goes to a log file a person can follow. A long step dies with
//! toolu on Ctrl-C.

use std::path::Path;
use std::time::Duration;

use toolu_protocol::exit::Exit;
use toolu_runtime::process::file::{FileSpec, run_to_file};
use toolu_runtime::process::guard::GroupGuard;
use toolu_runtime::process::{Spec, run};

use crate::step::{Failure, describe};

/// How long a short `limactl` command may take; `stop` waits for the guest.
const SHORT: Duration = Duration::from_secs(180);
/// How long a stopped long step has between `SIGTERM` and `SIGKILL`.
const GRACE: Duration = Duration::from_secs(10);

/// Run `argv` and return its stdout.
///
/// # Errors
/// Exit 69 when it cannot start, 1 when it fails, naming its stderr.
pub(crate) fn query(argv: &[String]) -> Result<String, Failure> {
  let mut spec = Spec::new(argv.iter().cloned());
  spec.timeout = SHORT;
  let output = run(&spec).map_err(|err| Failure::new(Exit::Unavailable, describe(&err)))?;
  if output.exit_code == 0 && !output.timed_out {
    return Ok(output.stdout);
  }
  let why = output.stderr.trim();
  let why = if output.timed_out {
    "it timed out"
  } else if why.is_empty() {
    "no message"
  } else {
    why
  };
  Err(Failure::new(
    Exit::Failure,
    format!("`{}` failed: {why}", words(argv)),
  ))
}

/// Run `argv` from `cwd` with its output in `log`, for at most `timeout`.
///
/// # Errors
/// Exit 69 when it cannot start, 1 when it fails or times out, naming the log.
pub(crate) fn logged(
  argv: &[String],
  cwd: &Path,
  log: &Path,
  timeout: Duration,
) -> Result<(), Failure> {
  let spec = FileSpec {
    argv: argv.to_vec(),
    cwd: Some(cwd.to_path_buf()),
    env: None,
    out: log.to_path_buf(),
    timeout: Some(timeout),
    grace: GRACE,
  };
  let guard = GroupGuard::install().ok();
  let output = run_to_file(&spec, &mut |group| {
    if let Some(guard) = &guard {
      guard.arm(group);
    }
  })
  .map_err(|err| Failure::new(Exit::Unavailable, describe(&err)))?;
  if output.exit_code == 0 && !output.timed_out {
    return Ok(());
  }
  let how = if output.timed_out {
    "timed out".to_owned()
  } else {
    format!("exited {}", output.exit_code)
  };
  Err(Failure::new(
    Exit::Failure,
    format!("`{}` {how}; see {}", words(argv), log.display()),
  ))
}

/// `argv` as one line for a message, without long script arguments.
fn words(argv: &[String]) -> String {
  argv
    .iter()
    .take_while(|word| !word.contains('\n'))
    .cloned()
    .collect::<Vec<_>>()
    .join(" ")
}

#[cfg(test)]
#[path = "tests/lima_process_test.rs"]
mod tests;
