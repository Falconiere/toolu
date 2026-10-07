//! A command whose stdout and stderr share one file (`runUnmanagedCheck` in
//! `packages/toolu-core/src/ledger/ledger-check.ts`): its own process group,
//! stdin on `/dev/null`, and an optional deadline. Past the deadline the group
//! gets `SIGTERM`, then `SIGKILL` once the grace period ends, and a last
//! `SIGKILL` after the leader exits. The run is over when the leader exits, as
//! Bun's `proc.exited` is; descendants that left the group, or outlive an
//! unbounded run, keep running. This function does not arm a `GroupGuard`.
//! The caller does, from `on_spawn`, when the run should die with this process.

use std::fs::File;
use std::os::unix::process::CommandExt as _;
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use nix::sys::signal::Signal;

use super::{RunError, exit_code, group};
use crate::env::Env;

/// The wait between two looks at a running leader.
const POLL: Duration = Duration::from_millis(25);

/// One command to run into a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileSpec {
  /// The program and its arguments.
  pub argv: Vec<String>,
  /// The working directory; the caller's when `None`.
  pub cwd: Option<PathBuf>,
  /// The whole environment of the child; the caller's when `None`.
  pub env: Option<Env>,
  /// The file both streams are written to; created or truncated.
  pub out: PathBuf,
  /// How long the command may run; unbounded when `None`.
  pub timeout: Option<Duration>,
  /// How long the group has between `SIGTERM` and `SIGKILL`.
  pub grace: Duration,
}

/// How a file run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileOutput {
  /// The leader's id, which is also its process-group id.
  pub pid: u32,
  /// The leader's exit status, or 128 plus the signal that ended it.
  pub exit_code: i32,
  /// The deadline passed and the group was stopped.
  pub timed_out: bool,
}

/// Run `spec`, calling `on_spawn` with the group id as soon as the leader started.
///
/// # Errors
/// [`RunError`] when the argv is empty, the file cannot be created, the
/// program cannot start, or the leader cannot be waited for.
pub fn run_to_file(spec: &FileSpec, on_spawn: &mut dyn FnMut(u32)) -> Result<FileOutput, RunError> {
  let program = spec.argv.first().filter(|program| !program.is_empty());
  let program = program.ok_or(RunError::EmptyArgv)?;
  let out = File::create(&spec.out)
    .map_err(|err| RunError::Spawn(format!("{}: {err}", spec.out.display())))?;
  let err = out
    .try_clone()
    .map_err(|err| RunError::Spawn(format!("{}: {err}", spec.out.display())))?;
  let mut command = Command::new(program);
  command
    .args(spec.argv.iter().skip(1))
    .process_group(0)
    .stdin(Stdio::null())
    .stdout(out)
    .stderr(err);
  if let Some(cwd) = &spec.cwd {
    command.current_dir(cwd);
  }
  if let Some(env) = &spec.env {
    command.env_clear().envs(env.vars());
  }
  let mut child = super::guard::unblocked(|| command.spawn())
    .map_err(|err| RunError::Spawn(format!("{program}: {err}")))?;
  let pid = child.id();
  on_spawn(pid);
  let deadline = spec
    .timeout
    .and_then(|timeout| Instant::now().checked_add(timeout));
  if let Some(status) = wait_until(&mut child, deadline)? {
    return Ok(FileOutput {
      pid,
      exit_code: exit_code(status),
      timed_out: false,
    });
  }
  let status = stop(&mut child, pid, spec.grace)?;
  Ok(FileOutput {
    pid,
    exit_code: exit_code(status),
    timed_out: true,
  })
}

/// The leader's status once it exits, or `None` when `deadline` passes first.
fn wait_until(
  child: &mut Child,
  deadline: Option<Instant>,
) -> Result<Option<ExitStatus>, RunError> {
  let Some(deadline) = deadline else {
    return child
      .wait()
      .map(Some)
      .map_err(|err| RunError::Wait(err.to_string()));
  };
  loop {
    if let Some(status) = reap(child)? {
      return Ok(Some(status));
    }
    let now = Instant::now();
    if now >= deadline {
      return Ok(None);
    }
    std::thread::sleep(POLL.min(deadline - now));
  }
}

/// `SIGTERM` to the group, `SIGKILL` after `grace` when the leader still runs,
/// and a final `SIGKILL` for the members that outlived the leader.
fn stop(child: &mut Child, pid: u32, grace: Duration) -> Result<ExitStatus, RunError> {
  // A group that is already gone is not an error; a refused signal leaves the wait to decide.
  let _sent = group::signal(pid, Signal::SIGTERM);
  let grace_end = Instant::now() + grace;
  let status = loop {
    if let Some(status) = reap(child)? {
      break status;
    }
    if Instant::now() >= grace_end {
      let _killed = group::signal(pid, Signal::SIGKILL);
      break child
        .wait()
        .map_err(|err| RunError::Wait(err.to_string()))?;
    }
    std::thread::sleep(POLL);
  };
  let _swept = group::signal(pid, Signal::SIGKILL);
  Ok(status)
}

fn reap(child: &mut Child) -> Result<Option<ExitStatus>, RunError> {
  child
    .try_wait()
    .map_err(|err| RunError::Wait(err.to_string()))
}

#[cfg(test)]
#[path = "tests/file_test.rs"]
mod tests;
