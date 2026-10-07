//! The `<file>.lock` protocol TypeScript writers use (`withLock` in
//! `state-io.ts`), so Rust and TypeScript read-merge-write cycles on one state
//! file never interleave. The lock is created exclusively (0600) and holds
//! `"<pid> <uuid>\n"`. A busy lock is polled every 10 ms; it is broken when its
//! holder's pid is gone or it is older than the stale age (2 s), by renaming it
//! aside and linking it back if a new holder took it meanwhile. After the
//! timeout (5 s) the closure runs unlocked, with a warning; a lock that cannot
//! be created at all runs it unlocked silently. Release, on drop (a panic
//! included), removes the lock only while it still holds our content.

use std::collections::hash_map::RandomState;
use std::hash::BuildHasher as _;
use std::io::Write as _;
use std::os::unix::fs::OpenOptionsExt as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use nix::errno::Errno;
use nix::sys::signal::kill;
use nix::unistd::Pid;

/// How long to wait for a busy lock, and when a held one counts as stale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LockOptions {
  /// Past this, the closure runs unlocked (5 s).
  pub timeout: Duration,
  /// A lock older than this is broken even if its holder lives (2 s).
  pub stale: Duration,
}

impl Default for LockOptions {
  fn default() -> LockOptions {
    LockOptions {
      timeout: Duration::from_secs(5),
      stale: Duration::from_secs(2),
    }
  }
}

const POLL: Duration = Duration::from_millis(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Take {
  Held,
  Busy,
  Unavailable,
}

/// `<file>.lock`.
pub fn lock_path(file: &Path) -> PathBuf {
  let mut lock = file.as_os_str().to_owned();
  lock.push(".lock");
  PathBuf::from(lock)
}

/// Runs `f` holding `<file>.lock`, as TypeScript's `withLock` does.
pub fn with_lock<T>(
  file: &Path,
  options: LockOptions,
  warnings: &mut Vec<String>,
  f: impl FnOnce() -> T,
) -> T {
  let lock = lock_path(file);
  let ours = format!("{} {}\n", std::process::id(), token());
  let deadline = Instant::now() + options.timeout;
  let mut state = try_lock(&lock, &ours);
  while state == Take::Busy {
    if Instant::now() >= deadline {
      warnings.push(format!(
        "state: lock {} still held; writing without it",
        lock.display()
      ));
      break;
    }
    break_if_stale(&lock, options.stale);
    state = try_lock(&lock, &ours);
    if state == Take::Busy {
      std::thread::sleep(POLL);
    }
  }
  let _release = Release {
    lock: &lock,
    ours: &ours,
    held: state == Take::Held,
  };
  f()
}

/// Removes the lock on drop if this call took it and still owns it.
struct Release<'a> {
  lock: &'a Path,
  ours: &'a str,
  held: bool,
}

impl Drop for Release<'_> {
  fn drop(&mut self) {
    if self.held && content(self.lock).as_deref() == Some(self.ours) {
      let _gone = std::fs::remove_file(self.lock);
    }
  }
}

fn try_lock(lock: &Path, ours: &str) -> Take {
  let created = std::fs::OpenOptions::new()
    .write(true)
    .create_new(true)
    .mode(0o600)
    .open(lock);
  match created {
    Ok(mut file) => {
      if file.write_all(ours.as_bytes()).is_ok() {
        return Take::Held;
      }
      let _gone = std::fs::remove_file(lock);
      Take::Unavailable
    }
    Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => Take::Busy,
    Err(_) => Take::Unavailable,
  }
}

fn content(lock: &Path) -> Option<String> {
  std::fs::read_to_string(lock).ok()
}

/// The holder named by `content` is a pid that no longer exists.
fn holder_dead(content: &str) -> bool {
  let first = content.split(' ').next().unwrap_or_default().trim();
  match first.parse::<i32>() {
    Ok(pid) if pid > 0 => kill(Pid::from_raw(pid), None) == Err(Errno::ESRCH),
    _ => false,
  }
}

/// Breaks the lock if its holder is dead or it is older than `stale`. Exactly
/// one waiter wins the rename; a fresh lock taken in between is linked back.
fn break_if_stale(lock: &Path, stale: Duration) {
  let (Some(held), Ok(meta)) = (content(lock), std::fs::metadata(lock)) else {
    return;
  };
  let age = meta
    .modified()
    .ok()
    .and_then(|at| SystemTime::now().duration_since(at).ok())
    .unwrap_or_default();
  if !holder_dead(&held) && age <= stale {
    return;
  }
  claim(lock, &held);
}

/// Takes the lock judged stale away by renaming it; a lock that turns out not
/// to be the one judged (`held`) was taken meanwhile and is linked back.
fn claim(lock: &Path, held: &str) {
  let mut claimed = lock.as_os_str().to_owned();
  claimed.push(format!(".{}.broken", token()));
  let claimed = PathBuf::from(claimed);
  if std::fs::rename(lock, &claimed).is_err() {
    return;
  }
  if content(&claimed).as_deref() != Some(held) {
    let _relinked = std::fs::hard_link(&claimed, lock);
  }
  let _gone = std::fs::remove_file(&claimed);
}

/// A random version 4 UUID, as `crypto.randomUUID()` prints one.
pub fn token() -> String {
  let half = |salt: u8| RandomState::new().hash_one((salt, std::process::id(), SystemTime::now()));
  let (high, low) = (half(0), half(1));
  let high = (high & !0xf000) | 0x4000;
  let low = (low & !(0b11 << 62)) | (0b10 << 62);
  format!(
    "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
    high >> 32,
    (high >> 16) & 0xffff,
    high & 0xffff,
    low >> 48,
    low & 0xffff_ffff_ffff
  )
}

#[cfg(test)]
#[path = "tests/lock_test.rs"]
mod tests;
