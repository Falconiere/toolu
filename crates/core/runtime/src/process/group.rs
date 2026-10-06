//! Process groups (`packages/toolu-core/src/process/process-group.ts`): signal a
//! detached command's whole group, tell whether a live member remains, and stop
//! it gently, then forcibly.

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use nix::errno::Errno;
use nix::sys::signal::{Signal, killpg};
use nix::unistd::Pid;

/// The wait between two looks at a group being stopped.
const POLL: Duration = Duration::from_millis(25);
/// How long a group gets to exit after `SIGTERM`.
const TERMINATE_GRACE: Duration = Duration::from_millis(250);
/// How long a group gets to exit after `SIGKILL`.
const KILL_GRACE: Duration = Duration::from_secs(1);

fn group_pid(group: u32) -> Result<Pid, String> {
  match i32::try_from(group) {
    Ok(raw) if raw > 0 => Ok(Pid::from_raw(raw)),
    Ok(_) | Err(_) => Err(format!("{group} is not a process-group id")),
  }
}

/// Sends `signal` to every process of `group`; a group that already exited is success.
///
/// # Errors
/// When `group` is not a positive process id, or the signal cannot be sent.
pub fn signal(group: u32, signal: Signal) -> Result<(), String> {
  match killpg(group_pid(group)?, signal) {
    Ok(()) | Err(Errno::ESRCH) => Ok(()),
    Err(errno) => Err(format!("cannot signal process group {group}: {errno}")),
  }
}

/// Whether `group` holds a process that is not a zombie. The process table
/// decides when it can be read; otherwise a signal probe does.
pub fn alive(group: u32) -> bool {
  let Ok(pid) = group_pid(group) else {
    return false;
  };
  match killpg(pid, None) {
    Err(Errno::ESRCH) => false,
    Ok(()) | Err(_) => listed_live(group).unwrap_or(true),
  }
}

/// `ps -axo pgid=,stat=`: whether a row of `group` has a non-zombie state, or
/// `None` when `ps` cannot be run.
fn listed_live(group: u32) -> Option<bool> {
  let table = Command::new("ps")
    .args(["-axo", "pgid=,stat="])
    .stdin(Stdio::null())
    .stderr(Stdio::null())
    .output()
    .ok()
    .filter(|table| table.status.success())?;
  let text = String::from_utf8_lossy(&table.stdout);
  let wanted = group.to_string();
  Some(text.lines().any(|row| {
    let mut columns = row.split_whitespace();
    columns.next() == Some(wanted.as_str())
      && columns.next().is_some_and(|stat| !stat.starts_with('Z'))
  }))
}

fn dead_by(group: u32, deadline: Instant) -> bool {
  loop {
    if !alive(group) {
      return true;
    }
    let now = Instant::now();
    if now >= deadline {
      return false;
    }
    std::thread::sleep(POLL.min(deadline - now));
  }
}

/// Stops `group`: `SIGTERM`, a grace period, then `SIGKILL`, and checks that no
/// live member remains.
///
/// # Errors
/// When a signal cannot be sent, or a member survives `SIGKILL`.
pub fn terminate(group: u32) -> Result<(), String> {
  signal(group, Signal::SIGTERM)?;
  if dead_by(group, Instant::now() + TERMINATE_GRACE) {
    return Ok(());
  }
  signal(group, Signal::SIGKILL)?;
  if dead_by(group, Instant::now() + KILL_GRACE) {
    Ok(())
  } else {
    Err(format!("process group {group} survived SIGKILL"))
  }
}

#[cfg(test)]
#[path = "tests/group_test.rs"]
mod tests;
