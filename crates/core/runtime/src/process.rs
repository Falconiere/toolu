//! Bounded subprocesses (`packages/toolu-core/src/process/run-command.ts`): one
//! command in its own process group, both streams drained into one byte budget,
//! and a deadline after which the whole group is terminated. Only this module
//! spawns processes (rule 14).
//!
//! - `exec`: replace this process with another program, which keeps the
//!   terminal, the streams and the exit status (`toolu sandbox run`);
//! - `file`: a command whose stdout and stderr share one file (the plan
//!   ledger's unmanaged check);
//! - `guard`: a detached group that dies with its runner on SIGINT, SIGTERM or
//!   SIGHUP;
//! - `usage`: this process's own peak memory and CPU time.

pub mod commands;
mod drain;
/// Replace the current process with a command.
pub mod exec;
/// Run a command with combined output in one file.
pub mod file;
/// Signal and inspect Unix process groups.
pub mod group;
/// Guard detached process groups across signals.
pub mod guard;
mod pipe;
/// Read process resource usage.
pub mod usage;

use std::os::unix::process::{CommandExt as _, ExitStatusExt as _};
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::env::Env;
use drain::{Budget, Drain};

/// Receives every stdout or stderr chunk as it is drained, before output truncation.
pub type ChunkObserver = Arc<dyn Fn(&[u8]) + Send + Sync>;

/// The deadline when a caller sets none.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
/// The bytes of stdout and stderr kept, together, when a caller sets no budget.
const DEFAULT_MAX_OUTPUT_BYTES: usize = 1_048_576;
/// Grace for final stream draining, feeder completion, and canceled worker cleanup.
const FINAL_DRAIN: Duration = Duration::from_millis(250);
/// The wait between two looks at a running command.
const POLL: Duration = Duration::from_millis(10);
/// The wait between two looks at a group whose leader exited.
const GROUP_POLL: Duration = Duration::from_millis(250);

/// When a run is over, short of its deadline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wait {
  /// The child exited, its streams closed and its process group has no live member.
  Group,
  /// The child exited, its output streams closed and stdin feeding completed;
  /// a process it started with its streams redirected is left running.
  Streams,
}

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
  /// When the run is over; the deadline terminates the whole group either way.
  pub wait: Wait,
  /// The interval between nonblocking waits; shorter for latency-sensitive commands.
  pub poll_interval: Duration,
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
      wait: Wait::Group,
      poll_interval: POLL,
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
  /// Stdout exactly as the child wrote it, for output that is not text.
  pub stdout_bytes: Vec<u8>,
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
  /// The deadline lies past what the clock can represent.
  TimeoutTooLong,
  /// The program could not be started.
  Spawn(String),
  /// The child could not be waited for.
  Wait(String),
  /// Stdin could not be written for a reason other than the child closing it.
  Stdin(String),
  /// The spawn callback of [`run_with`] failed; the group was stopped.
  Callback(String),
}

/// Run `spec` and return once the child exited and its group has no live
/// member (or, with [`Wait::Streams`], once its output streams closed and stdin
/// feeding completed), or the
/// deadline passed and the group was terminated.
///
/// # Errors
/// [`RunError`] when the spec is unusable, the program cannot start, or the
/// child cannot be waited for.
pub fn run(spec: &Spec) -> Result<Output, RunError> {
  run_with(spec, &mut |_| Ok(()))
}

/// [`run`], calling `on_spawn` with the child's process-group id once it
/// started and before its stdin is written (`runCommand`'s `onSpawn`). When the
/// callback fails, the group is stopped and its error returned. Arming a
/// `GroupGuard` belongs in that callback. `run` passes an empty one.
///
/// # Errors
/// As [`run`], plus [`RunError::Callback`].
pub fn run_with(
  spec: &Spec,
  on_spawn: &mut dyn FnMut(u32) -> Result<(), String>,
) -> Result<Output, RunError> {
  run_with_impl(spec, on_spawn, None)
}

/// [`run_with`] with an observer for every raw output chunk from both streams.
///
/// # Errors
/// As [`run_with`].
pub fn run_with_chunks(
  spec: &Spec,
  on_spawn: &mut dyn FnMut(u32) -> Result<(), String>,
  observer: ChunkObserver,
) -> Result<Output, RunError> {
  run_with_impl(spec, on_spawn, Some(observer))
}

fn run_with_impl(
  spec: &Spec,
  on_spawn: &mut dyn FnMut(u32) -> Result<(), String>,
  observer: Option<ChunkObserver>,
) -> Result<Output, RunError> {
  let program = spec.argv.first().filter(|program| !program.is_empty());
  let program = program.ok_or(RunError::EmptyArgv)?;
  if spec.timeout.is_zero() {
    return Err(RunError::ZeroTimeout);
  }
  let started = Instant::now();
  let deadline = started
    .checked_add(spec.timeout)
    .ok_or(RunError::TimeoutTooLong)?;
  let mut child = guard::unblocked(|| command(program, spec).spawn())
    .map_err(|err| RunError::Spawn(format!("{program}: {err}")))?;
  let pid = child.id();
  notify_spawn(&mut child, on_spawn)?;
  let (stdin, stdout_pipe, stderr_pipe) =
    pipe::take_pipes(&mut child).map_err(|err| pipe::failed_setup(&mut child, pid, &err))?;
  let cancel = Arc::new(AtomicBool::new(false));
  let feeder = pipe::feed(stdin, spec.stdin.clone(), Arc::clone(&cancel));
  let budget = Budget::new(spec.max_output_bytes);
  let mut stdout = Drain::start(stdout_pipe, &budget, observer.clone(), Arc::clone(&cancel));
  let mut stderr = Drain::start(stderr_pipe, &budget, observer, Arc::clone(&cancel));
  let settled = settle(
    &mut child,
    [&stdout, &stderr],
    feeder.as_ref(),
    deadline,
    spec,
  );
  let fed = pipe::finish_workers(&cancel, feeder, &mut stdout, &mut stderr, settled.is_ok());
  let (status, timed_out) = settled?;
  if let Some(Err(err)) = fed {
    return Err(RunError::Stdin(err));
  }
  Ok(Output {
    pid,
    stdout: stdout.text(),
    stdout_bytes: stdout.bytes(),
    stderr: stderr.text(),
    exit_code: exit_code(status),
    duration: started.elapsed(),
    timed_out,
    truncated: budget.truncated(),
  })
}

/// Spawn `spec` in its own process group and return its pid without waiting.
/// A reaper thread waits so a long-lived parent does not leave a zombie; when
/// the parent exits first, the child is reparented and keeps running.
///
/// # Errors
/// [`RunError::EmptyArgv`] or [`RunError::Spawn`].
pub fn detach(spec: &Spec) -> Result<u32, RunError> {
  let mut child = detached_child(spec)?;
  let pid = child.id();
  std::thread::spawn(move || {
    let _reaped = child.wait();
  });
  Ok(pid)
}

/// Start a one-shot child in its own group and leave it for init to reap when
/// this short-lived caller exits. The caller must never wait for the child.
///
/// # Errors
/// [`RunError::EmptyArgv`] or [`RunError::Spawn`].
pub fn spawn_orphan(spec: &Spec) -> Result<u32, RunError> {
  let child = detached_child(spec)?;
  Ok(child.id())
}

fn detached_child(spec: &Spec) -> Result<Child, RunError> {
  let program = spec
    .argv
    .first()
    .filter(|program| !program.is_empty())
    .ok_or(RunError::EmptyArgv)?;
  let mut command = Command::new(program);
  command
    .args(spec.argv.iter().skip(1))
    .process_group(0)
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::null());
  if let Some(cwd) = &spec.cwd {
    command.current_dir(cwd);
  }
  if let Some(env) = &spec.env {
    command.env_clear().envs(env.vars());
  }
  guard::unblocked(|| command.spawn()).map_err(|err| RunError::Spawn(format!("{program}: {err}")))
}

/// Invoke the spawn callback, stopping and reaping the child if it fails.
fn notify_spawn(
  child: &mut Child,
  on_spawn: &mut dyn FnMut(u32) -> Result<(), String>,
) -> Result<(), RunError> {
  if let Err(err) = on_spawn(child.id()) {
    let _stopped = group::terminate_reaping(child.id(), &mut || {
      let _reaped = child.try_wait();
    });
    let _waited = child.wait();
    return Err(RunError::Callback(err));
  }
  Ok(())
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

/// Waits for the child, all pipe workers and (with [`Wait::Group`]) its group until
/// `deadline`; past it, terminates the group and gives the streams a last moment to close.
fn settle(
  child: &mut Child,
  drains: [&Drain; 2],
  feeder: Option<&JoinHandle<Result<(), String>>>,
  deadline: Instant,
  spec: &Spec,
) -> Result<(ExitStatus, bool), RunError> {
  let pid = child.id();
  let mut status = None;
  let mut next_group_check = Instant::now();
  loop {
    if status.is_none() {
      status = child.try_wait().map_err(|err| abandon(pid, &err))?;
    }
    let now = Instant::now();
    let streams_done = drains.iter().all(|drain| drain.finished());
    let pipes_done = streams_done && feeder.is_none_or(JoinHandle::is_finished);
    if let Some(status) = status.filter(|_| pipes_done && spec.wait == Wait::Streams) {
      return Ok((status, false));
    }
    if let Some(status) = status.filter(|_| pipes_done && now >= next_group_check) {
      if !group::alive(pid) {
        return Ok((status, false));
      }
      next_group_check = now + GROUP_POLL;
    }
    if now >= deadline {
      break;
    }
    std::thread::sleep(spec.poll_interval.min(deadline - now));
  }
  let terminated = group::terminate_reaping(pid, &mut || {
    if status.is_none() {
      status = child.try_wait().ok().flatten();
    }
  });
  let status = match status {
    Some(status) => status,
    None => child.wait().map_err(|err| abandon(pid, &err))?,
  };
  terminated.map_err(RunError::Wait)?;
  let drain_deadline = Instant::now() + FINAL_DRAIN;
  while Instant::now() < drain_deadline && !drains.iter().all(|drain| drain.finished()) {
    std::thread::sleep(POLL);
  }
  Ok((status, true))
}

/// A child that cannot be waited for: its group is stopped before the error returns.
fn abandon(pid: u32, err: &std::io::Error) -> RunError {
  let stopped = group::terminate(pid)
    .err()
    .map(|why| format!("; {why}"))
    .unwrap_or_default();
  RunError::Wait(format!("{err}{stopped}"))
}

/// The exit status, or 128 plus the signal that ended the child.
pub(crate) fn exit_code(status: ExitStatus) -> i32 {
  match (status.code(), status.signal()) {
    (Some(code), _) => code,
    (None, Some(signal)) => 128 + signal,
    (None, None) => 128,
  }
}

#[cfg(test)]
#[path = "tests/process_test.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/escaped_test.rs"]
mod escaped_tests;
