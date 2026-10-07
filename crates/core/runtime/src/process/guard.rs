//! A detached process group that dies with its runner (`guardGroup` in
//! `packages/toolu-core/src/ledger/ledger-check.ts`, `guardOwnedGroup` in
//! `resources/jobs.ts`). While a group is armed, SIGINT, SIGTERM or SIGHUP to
//! this process first SIGKILLs the group, then ends the process by the same
//! signal. Catching a signal needs `unsafe` in Rust, so the three signals are
//! blocked instead and one `sigwait` thread takes them for the life of the
//! process. A child inherits its spawning thread's mask, so every spawn runs
//! inside `unblocked`, and the child starts with the signals unblocked. A
//! guard dropped while unwinding kills its group too.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, Ordering};

use nix::sys::signal::{SigSet, SigmaskHow, Signal, pthread_sigmask, raise};

use super::group;

/// The armed group, or 0.
static ARMED: AtomicU32 = AtomicU32::new(0);
/// Whether the watcher started, once per process.
static WATCHER: OnceLock<Result<(), String>> = OnceLock::new();

/// The signals a runner forwards to its group.
fn forwarded() -> SigSet {
  let mut signals = SigSet::empty();
  for signal in [Signal::SIGINT, Signal::SIGTERM, Signal::SIGHUP] {
    signals.add(signal);
  }
  signals
}

/// SIGKILLs the armed group, if any; one that already exited is not an error.
fn kill_armed() {
  let armed = ARMED.swap(0, Ordering::SeqCst);
  if armed != 0 {
    let _killed = group::signal(armed, Signal::SIGKILL);
  }
}

/// Takes each forwarded signal: kills the armed group, then lets the signal end
/// the process from this thread, the only one that no longer blocks it.
fn watch(signals: SigSet) {
  loop {
    let Ok(signal) = signals.wait() else {
      continue;
    };
    kill_armed();
    if pthread_sigmask(SigmaskHow::SIG_UNBLOCK, Some(&signals), None).is_ok() {
      let _raised = raise(signal);
    }
  }
}

fn start() -> Result<(), String> {
  let signals = forwarded();
  pthread_sigmask(SigmaskHow::SIG_BLOCK, Some(&signals), None)
    .map_err(|errno| format!("cannot block the runner's signals: {errno}"))?;
  std::thread::Builder::new()
    .name("toolu-group-guard".to_owned())
    .spawn(move || watch(signals))
    .map(|_| ())
    .map_err(|err| format!("cannot start the signal watcher: {err}"))
}

/// Runs `spawn` with the forwarded signals unblocked in this thread, then
/// restores the thread's mask, so a child spawned inside starts with them
/// unblocked. Without an installed guard it only runs `spawn`.
pub(crate) fn unblocked<T>(spawn: impl FnOnce() -> T) -> T {
  if !WATCHER.get().is_some_and(Result::is_ok) {
    return spawn();
  }
  let mut previous = SigSet::empty();
  let changed = pthread_sigmask(
    SigmaskHow::SIG_UNBLOCK,
    Some(&forwarded()),
    Some(&mut previous),
  )
  .is_ok();
  let spawned = spawn();
  if changed {
    let _restored = pthread_sigmask(SigmaskHow::SIG_SETMASK, Some(&previous), None);
  }
  spawned
}

/// The runner's hold on one detached group.
#[derive(Debug)]
pub struct GroupGuard(());

impl GroupGuard {
  /// Blocks SIGINT, SIGTERM and SIGHUP in the calling thread (and the threads
  /// it starts afterwards) and starts the watcher once per process. Call it
  /// before spawning the group and before starting other threads.
  ///
  /// # Errors
  /// When the signals cannot be blocked or the watcher cannot start.
  pub fn install() -> Result<GroupGuard, String> {
    WATCHER.get_or_init(start).clone()?;
    Ok(GroupGuard(()))
  }

  /// Arms the guard for `group`.
  pub fn arm(&self, group: u32) {
    ARMED.store(group, Ordering::SeqCst);
  }

  /// The armed group, or 0.
  pub fn armed(&self) -> u32 {
    ARMED.load(Ordering::SeqCst)
  }
}

impl Drop for GroupGuard {
  /// Disarms; a guard dropped while unwinding kills its group first.
  fn drop(&mut self) {
    if std::thread::panicking() {
      kill_armed();
    } else {
      ARMED.store(0, Ordering::SeqCst);
    }
  }
}

#[cfg(test)]
#[path = "tests/guard_test.rs"]
mod tests;
