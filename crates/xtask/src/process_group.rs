//! A child run as the leader of its own process group, so a deadline or an exit takes
//! every descendant with it and none keeps a captured pipe open.

use std::io::{self, Read};
use std::process::{Child, Command, ExitStatus};
use std::thread::{JoinHandle, sleep, spawn};
use std::time::{Duration, Instant};

use nix::errno::Errno;
use nix::sys::signal::{Signal, killpg};
use nix::unistd::Pid;

/// The wait between two looks at a running child.
const POLL: Duration = Duration::from_millis(20);

/// Spawns tried again when the executable is still open for writing.
const BUSY_RETRIES: usize = 20;
/// The wait between two such spawns.
const BUSY_WAIT: Duration = Duration::from_millis(25);

/// Run `attempt` again while it fails with `ETXTBSY`: on Linux a freshly written
/// executable cannot be executed while a concurrent `fork` still holds its write
/// descriptor, which clears within milliseconds. The first other result, or the
/// last `ETXTBSY` once the retries are spent, is returned.
pub(crate) fn retry_busy<T>(mut attempt: impl FnMut() -> io::Result<T>) -> io::Result<T> {
  let mut retries = 0;
  loop {
    match attempt() {
      Err(err) if err.raw_os_error() == Some(Errno::ETXTBSY as i32) && retries < BUSY_RETRIES => {
        retries += 1;
        sleep(BUSY_WAIT);
      }
      other => return other,
    }
  }
}

/// `command.spawn()`, tried again while the program is busy (see [`retry_busy`]).
pub(crate) fn spawn_retrying(command: &mut Command) -> io::Result<Child> {
  retry_busy(|| command.spawn())
}

/// Read `stream` to its end on a thread, so a full pipe never stalls the child.
pub(crate) fn drain<R: Read + Send + 'static>(mut stream: R) -> JoinHandle<String> {
  spawn(move || {
    let mut bytes = Vec::new();
    // A read that fails mid-stream keeps what arrived: the transcript is evidence.
    stream.read_to_end(&mut bytes).ok();
    String::from_utf8_lossy(&bytes).into_owned()
  })
}

/// What a [`drain`] thread read; a reader that panicked is an error.
pub(crate) fn collect(handle: JoinHandle<String>) -> Result<String, String> {
  handle.join().map_err(|payload| {
    let why = payload
      .downcast_ref::<&str>()
      .copied()
      .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
      .unwrap_or("unknown cause");
    format!("an output reader panicked: {why}")
  })
}

/// `SIGKILL` to every process of the group `leader` leads. A group that is gone is success,
/// and so is `EPERM`: the group is ours, so macOS only says it for one holding nothing
/// but killed processes that init has not reaped yet.
pub(crate) fn kill(leader: u32) -> Result<(), String> {
  let raw = i32::try_from(leader).map_err(|err| format!("process group {leader}: {err}"))?;
  match killpg(Pid::from_raw(raw), Signal::SIGKILL) {
    Ok(()) | Err(Errno::ESRCH | Errno::EPERM) => Ok(()),
    Err(errno) => Err(format!("cannot kill process group {leader}: {errno}")),
  }
}

/// Wait for `child`, calling `tick` before every look and once after it exits. `Some`
/// is its status; past `timeout` its whole group is killed and reaped, and the result
/// is `None`. `child` must lead its group (`CommandExt::process_group(0)`).
pub(crate) fn wait(
  child: &mut Child,
  timeout: Duration,
  tick: &mut dyn FnMut(),
) -> Result<Option<ExitStatus>, String> {
  let deadline = Instant::now() + timeout;
  loop {
    tick();
    if let Some(status) = child.try_wait().map_err(|err| err.to_string())? {
      tick();
      return Ok(Some(status));
    }
    if Instant::now() >= deadline {
      kill(child.id())?;
      child.wait().map_err(|err| err.to_string())?;
      return Ok(None);
    }
    sleep(POLL);
  }
}

#[cfg(test)]
#[path = "tests/process_group_test.rs"]
mod tests;
