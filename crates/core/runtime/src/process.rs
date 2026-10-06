//! Bounded subprocesses (`packages/toolu-core/src/process/run-command.ts`): one
//! command in its own process group, both streams drained into one byte budget,
//! and a deadline after which the whole group is terminated. Only this module
//! spawns processes (rule 14).

pub mod commands;
mod drain;
pub mod group;

use std::io::{ErrorKind, Write as _};
use std::os::unix::process::{CommandExt as _, ExitStatusExt as _};
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::env::Env;
use drain::{Budget, Drain};

/// The deadline when a caller sets none.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
/// The bytes of stdout and stderr kept, together, when a caller sets no budget.
const DEFAULT_MAX_OUTPUT_BYTES: usize = 1_048_576;
/// How long the streams may take to close once the group was terminated.
const FINAL_DRAIN: Duration = Duration::from_millis(250);
/// The wait between two looks at a running command.
const POLL: Duration = Duration::from_millis(10);
/// The wait between two looks at a group whose leader exited.
const GROUP_POLL: Duration = Duration::from_millis(250);

/// One command to run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spec {
  /// The program and its arguments.
  pub argv: Vec<String>,
  /// The working directory; the caller's when `None`.
  pub cwd: Option<PathBuf>,
  /// The whole environment of the child; the caller's when `None`.
  pub env: Option<Env>,
  /// Bytes written to the child's stdin, which is then closed.
  pub stdin: Vec<u8>,
  /// How long the command and its group may run.
  pub timeout: Duration,
  /// The bytes of stdout and stderr kept, together.
  pub max_output_bytes: usize,
}

impl Spec {
  /// `argv` with no stdin, the caller's directory and environment, and the defaults.
  pub fn new<I: IntoIterator<Item = S>, S: Into<String>>(argv: I) -> Spec {
    Spec {
      argv: argv.into_iter().map(Into::into).collect(),
      cwd: None,
      env: None,
      stdin: Vec::new(),
      timeout: DEFAULT_TIMEOUT,
      max_output_bytes: DEFAULT_MAX_OUTPUT_BYTES,
    }
  }
}

/// What a command left behind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
  /// The child's id, which is also its process-group id.
  pub pid: u32,
  /// Stdout, decoded lossily.
  pub stdout: String,
  /// Stderr, decoded lossily.
  pub stderr: String,
  /// The exit status, or 128 plus the signal that ended the child.
  pub exit_code: i32,
  /// From spawn to the end of the wait.
  pub duration: Duration,
  /// The deadline passed and the group was terminated.
  pub timed_out: bool,
  /// Output past the budget was dropped.
  pub truncated: bool,
}

/// Why a command could not run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunError {
  /// The argv is empty or its program is empty.
  EmptyArgv,
  /// The timeout is zero.
  ZeroTimeout,
  /// The program could not be started.
  Spawn(String),
  /// The child could not be waited for.
  Wait(String),
  /// Stdin could not be written for a reason other than the child closing it.
  Stdin(String),
}

/// Run `spec` and return once the child exited and its group has no live
/// member, or the deadline passed and the group was terminated.
///
/// # Errors
/// [`RunError`] when the spec is unusable, the program cannot start, or the
/// child cannot be waited for.
pub fn run(spec: &Spec) -> Result<Output, RunError> {
  let program = spec.argv.first().filter(|program| !program.is_empty());
  let program = program.ok_or(RunError::EmptyArgv)?;
  if spec.timeout.is_zero() {
    return Err(RunError::ZeroTimeout);
  }
  let started = Instant::now();
  let mut child = command(program, spec)
    .spawn()
    .map_err(|err| RunError::Spawn(format!("{program}: {err}")))?;
  let pid = child.id();
  let feeder = feed(&mut child, spec.stdin.clone());
  let budget = Budget::new(spec.max_output_bytes);
  let stdout = Drain::start(child.stdout.take(), &budget);
  let stderr = Drain::start(child.stderr.take(), &budget);
  let deadline = started + spec.timeout;
  let (status, timed_out) = settle(&mut child, pid, [&stdout, &stderr], deadline)?;
  let fed = feeder.map(JoinHandle::join);
  if let Some(Ok(Err(err))) = fed {
    return Err(RunError::Stdin(err));
  }
  Ok(Output {
    pid,
    stdout: stdout.text(),
    stderr: stderr.text(),
    exit_code: exit_code(status),
    duration: started.elapsed(),
    timed_out,
    truncated: budget.truncated(),
  })
}

fn command(program: &str, spec: &Spec) -> Command {
  let mut command = Command::new(program);
  command
    .args(spec.argv.iter().skip(1))
    .process_group(0)
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
  if let Some(cwd) = &spec.cwd {
    command.current_dir(cwd);
  }
  if let Some(env) = &spec.env {
    command.env_clear().envs(env.vars());
  }
  command
}

/// Writes `input` to the child's stdin on a thread, then closes it. A child
/// that closed its stdin first is not an error.
fn feed(child: &mut Child, input: Vec<u8>) -> Option<JoinHandle<Result<(), String>>> {
  let mut pipe = child.stdin.take()?;
  Some(std::thread::spawn(move || match pipe.write_all(&input) {
    Err(err) if err.kind() != ErrorKind::BrokenPipe => Err(err.to_string()),
    Ok(()) | Err(_) => Ok(()),
  }))
}

/// Waits for the child, its group and both streams until `deadline`; past it,
/// terminates the group and gives the streams a last moment to close.
fn settle(
  child: &mut Child,
  pid: u32,
  drains: [&Drain; 2],
  deadline: Instant,
) -> Result<(ExitStatus, bool), RunError> {
  let mut status = None;
  let mut next_group_check = Instant::now();
  loop {
    if status.is_none() {
      status = child
        .try_wait()
        .map_err(|err| RunError::Wait(err.to_string()))?;
    }
    let now = Instant::now();
    let streams_done = drains.iter().all(|drain| drain.finished());
    if let Some(status) = status.filter(|_| streams_done && now >= next_group_check) {
      if !group::alive(pid) {
        return Ok((status, false));
      }
      next_group_check = now + GROUP_POLL;
    }
    if now >= deadline {
      break;
    }
    std::thread::sleep(POLL.min(deadline - now));
  }
  let terminated = group::terminate(pid);
  let status = match status {
    Some(status) => status,
    None => child
      .wait()
      .map_err(|err| RunError::Wait(err.to_string()))?,
  };
  terminated.map_err(RunError::Wait)?;
  let drain_deadline = Instant::now() + FINAL_DRAIN;
  while Instant::now() < drain_deadline && !drains.iter().all(|drain| drain.finished()) {
    std::thread::sleep(POLL);
  }
  Ok((status, true))
}

fn exit_code(status: ExitStatus) -> i32 {
  match (status.code(), status.signal()) {
    (Some(code), _) => code,
    (None, Some(signal)) => 128 + signal,
    (None, None) => 128,
  }
}

#[cfg(test)]
#[path = "tests/process_test.rs"]
mod tests;
