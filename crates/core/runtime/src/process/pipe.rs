//! Nonblocking subprocess pipes and cancellable stdin feeding.

use std::io::{ErrorKind, Write as _};
use std::os::fd::AsFd;
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Instant;

use nix::fcntl::{FcntlArg, OFlag, fcntl};

use super::drain::Drain;
use super::{FINAL_DRAIN, POLL, RunError, group};

/// The three inherited child pipe endpoints.
pub(super) type Pipes = (Option<ChildStdin>, Option<ChildStdout>, Option<ChildStderr>);

/// Take and configure every present pipe before any worker starts.
pub(super) fn take_pipes(child: &mut Child) -> Result<Pipes, String> {
  let pipes = (child.stdin.take(), child.stdout.take(), child.stderr.take());
  if let Some(pipe) = &pipes.0 {
    nonblocking(pipe)?;
  }
  if let Some(pipe) = &pipes.1 {
    nonblocking(pipe)?;
  }
  if let Some(pipe) = &pipes.2 {
    nonblocking(pipe)?;
  }
  Ok(pipes)
}

fn nonblocking(pipe: &impl AsFd) -> Result<(), String> {
  let flags = fcntl(pipe, FcntlArg::F_GETFL).map_err(|err| err.to_string())?;
  fcntl(
    pipe,
    FcntlArg::F_SETFL(OFlag::from_bits_truncate(flags) | OFlag::O_NONBLOCK),
  )
  .map_err(|err| err.to_string())?;
  Ok(())
}

/// Stop and reap a child whose pipe launch setup failed.
pub(super) fn failed_setup(child: &mut Child, pid: u32, err: &str) -> RunError {
  let stopped = group::terminate_reaping(pid, &mut || {
    let _reaped = child.try_wait();
  })
  .err()
  .map(|why| format!("; {why}"))
  .unwrap_or_default();
  let reaped = child
    .wait()
    .err()
    .map(|why| format!("; {why}"))
    .unwrap_or_default();
  RunError::Spawn(format!("pipe setup failed: {err}{stopped}{reaped}"))
}

/// Writes input until complete, canceled, or the child closes stdin.
pub(super) fn feed(
  pipe: Option<ChildStdin>,
  input: Vec<u8>,
  cancel: Arc<AtomicBool>,
) -> Option<JoinHandle<Result<(), String>>> {
  let pipe = pipe?;
  if input.is_empty() {
    return None;
  }
  Some(std::thread::spawn(move || {
    write_input(pipe, &input, &cancel)
  }))
}

fn write_input(mut pipe: ChildStdin, input: &[u8], cancel: &AtomicBool) -> Result<(), String> {
  let mut remaining = input;
  while !remaining.is_empty() && !cancel.load(Ordering::Acquire) {
    match pipe.write(remaining) {
      Ok(0) => return Err("stdin write returned zero bytes".to_owned()),
      Ok(written) => {
        remaining = remaining
          .get(written..)
          .ok_or_else(|| "stdin write exceeded input length".to_owned())?;
      }
      Err(err) if err.kind() == ErrorKind::BrokenPipe => return Ok(()),
      Err(err) if matches!(err.kind(), ErrorKind::Interrupted | ErrorKind::WouldBlock) => {
        std::thread::sleep(POLL);
      }
      Err(err) => return Err(err.to_string()),
    }
  }
  Ok(())
}

/// Signal all workers and wait for them against one cleanup deadline.
pub(super) fn finish_workers(
  cancel: &AtomicBool,
  feeder: Option<JoinHandle<Result<(), String>>>,
  stdout: &mut Drain,
  stderr: &mut Drain,
  allow_feed_grace: bool,
) -> Option<Result<(), String>> {
  if allow_feed_grace {
    let grace_deadline = Instant::now() + FINAL_DRAIN;
    while feeder.as_ref().is_some_and(|handle| !handle.is_finished()) {
      let now = Instant::now();
      if now >= grace_deadline {
        break;
      }
      std::thread::sleep(POLL.min(grace_deadline - now));
    }
  }
  cancel.store(true, Ordering::Release);
  let deadline = Instant::now() + FINAL_DRAIN;
  let result = feeder.and_then(|handle| fed(handle, deadline));
  stdout.finish_by(deadline);
  stderr.finish_by(deadline);
  result
}

fn fed(feeder: JoinHandle<Result<(), String>>, deadline: Instant) -> Option<Result<(), String>> {
  while !feeder.is_finished() && Instant::now() < deadline {
    std::thread::sleep(POLL.min(deadline.saturating_duration_since(Instant::now())));
  }
  if feeder.is_finished() {
    feeder.join().ok()
  } else {
    None
  }
}

#[cfg(test)]
#[path = "tests/pipe_test.rs"]
mod tests;
