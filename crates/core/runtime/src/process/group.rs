//! Process groups (`packages/toolu-core/src/process/process-group.ts`): signal a
//! detached command's whole group, tell whether a live member remains, and stop
//! it gently, then forcibly.

use std::io::Read as _;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use nix::errno::Errno;
use nix::sys::signal::{Signal, kill, killpg};
use nix::unistd::Pid;

/// The wait between two looks at a group being stopped.
const POLL: Duration = Duration::from_millis(25);
/// How long a group gets to exit after `SIGTERM`.
const TERMINATE_GRACE: Duration = Duration::from_millis(250);
/// How long a group gets to exit after `SIGKILL`.
const KILL_GRACE: Duration = Duration::from_secs(1);
/// How long `ps` may take to list the process table.
const PS_TIMEOUT: Duration = Duration::from_secs(2);
/// The process-table bytes read from `ps`.
const PS_MAX_BYTES: u64 = 8 * 1024 * 1024;

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

/// Whether process `pid` exists (`processAlive` in `resources/lock.ts`): the
/// probe `kill(pid, 0)`, where a non-positive or out-of-range id is no process.
///
/// # Errors
/// Any probe failure but "no such process", such as another user's process.
pub fn pid_alive(pid: i64) -> Result<bool, String> {
  let Some(raw) = i32::try_from(pid).ok().filter(|raw| *raw > 0) else {
    return Ok(false);
  };
  match kill(Pid::from_raw(raw), None) {
    Ok(()) => Ok(true),
    Err(Errno::ESRCH) => Ok(false),
    Err(errno) => Err(format!("cannot probe process {pid}: {errno}")),
  }
}

/// Whether `group` holds a process that is not a zombie. The process table
/// decides when it can be read; otherwise a signal probe does.
pub fn alive(group: u32) -> bool {
  signal_probe(group) && listed_live(group).unwrap_or(true)
}

/// `kill(-group, 0)`: whether any process, zombies included, is in `group`.
fn signal_probe(group: u32) -> bool {
  match group_pid(group) {
    Ok(pid) => !matches!(killpg(pid, None), Err(Errno::ESRCH)),
    Err(_) => false,
  }
}

/// `ps -axo pgid=,stat=`: whether a row of `group` has a non-zombie state, or
/// `None` when `ps` cannot be run, fails, or takes longer than [`PS_TIMEOUT`].
fn listed_live(group: u32) -> Option<bool> {
  let mut ps = Command::new("ps")
    .args(["-axo", "pgid=,stat="])
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::null())
    .spawn()
    .ok()?;
  let mut stdout = ps.stdout.take()?;
  let reader = std::thread::spawn(move || {
    let mut table = Vec::new();
    stdout
      .by_ref()
      .take(PS_MAX_BYTES)
      .read_to_end(&mut table)
      .map(|_| table)
  });
  let deadline = Instant::now() + PS_TIMEOUT;
  let status = loop {
    match ps.try_wait() {
      Ok(Some(status)) => break status,
      Ok(None) if Instant::now() < deadline => std::thread::sleep(POLL),
      Ok(None) | Err(_) => {
        let _killed = ps.kill();
        let _reaped = ps.wait();
        return None;
      }
    }
  };
  let table = reader.join().ok()?.ok().filter(|_| status.success())?;
  let text = String::from_utf8_lossy(&table);
  let wanted = group.to_string();
  Some(text.lines().any(|row| {
    let mut columns = row.split_whitespace();
    columns.next() == Some(wanted.as_str())
      && columns.next().is_some_and(|stat| !stat.starts_with('Z'))
  }))
}

fn dead_by(group: u32, deadline: Instant, reap: &mut dyn FnMut()) -> bool {
  loop {
    reap();
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
  terminate_reaping(group, &mut || {})
}

/// [`terminate`], calling `reap` before each look at the group, so the caller
/// can reap its own child: an unreaped leader stays in the group as a zombie,
/// which only `ps` can tell from a live process.
pub(super) fn terminate_reaping(group: u32, reap: &mut dyn FnMut()) -> Result<(), String> {
  signal(group, Signal::SIGTERM)?;
  if dead_by(group, Instant::now() + TERMINATE_GRACE, reap) {
    return Ok(());
  }
  signal(group, Signal::SIGKILL)?;
  if dead_by(group, Instant::now() + KILL_GRACE, reap) {
    Ok(())
  } else {
    Err(format!("process group {group} survived SIGKILL"))
  }
}

#[cfg(test)]
#[path = "tests/group_test.rs"]
mod tests;
