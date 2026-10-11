//! The slot lock: `<state>.lock`, a directory made atomically by `mkdir`, with
//! `pid` and `since` files inside. A lock whose pid is dead, that is older than
//! `PB_LOCK_STALE_SECONDS` (600), or that has stayed incomplete for a second is
//! reclaimed once, unless the directory changed since it was judged; anything
//! else is `locked`. Only the directory this process made is ever removed on
//! release.

use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::Value;
use toolu_runtime::env::Env;
use toolu_runtime::process::group::pid_alive;

use crate::error::{Failure, Result};

/// A held slot lock, released on drop.
#[derive(Debug)]
pub(crate) struct SlotLock {
  path: PathBuf,
  identity: Option<(u64, u64)>,
}

fn now_secs() -> u64 {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map_or(0, |since| since.as_secs())
}

fn locked(pid: Option<u64>, since: Option<u64>) -> Failure {
  Failure::new("locked", "slot is held by another controller")
    .with("pid", pid.map_or(Value::Null, Value::from))
    .with("since", since.map_or(Value::Null, Value::from))
}

fn lock_error(err: &std::io::Error) -> Failure {
  Failure::new("api_error", format!("slot lock failed: {err}")).with("source", "lock")
}

/// A lock directory that vanished under us was taken by someone else.
fn claim_error(err: &std::io::Error) -> Failure {
  if err.kind() == std::io::ErrorKind::NotFound {
    locked(None, None)
  } else {
    lock_error(err)
  }
}

fn read_number(path: &Path) -> Option<u64> {
  let text = std::fs::read_to_string(path).ok()?;
  let text = text.trim();
  if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
    return None;
  }
  text.parse().ok()
}

enum Seen {
  Retry,
  Stale(Option<u64>, Option<u64>, (u64, u64)),
  Held(Option<u64>, Option<u64>),
}

impl SlotLock {
  /// Take the lock beside `state`, reclaiming a stale one once.
  pub(crate) fn acquire(state: &Path, env: &Env) -> Result<SlotLock> {
    let mut path = state.as_os_str().to_owned();
    path.push(".lock");
    let path = PathBuf::from(path);
    if let Some(parent) = path.parent() {
      std::fs::create_dir_all(parent).map_err(|err| lock_error(&err))?;
    }
    let stale_after = env
      .get("PB_LOCK_STALE_SECONDS")
      .unwrap_or("600")
      .parse::<u64>()
      .ok();
    for attempt in 0..2 {
      match std::fs::create_dir(&path) {
        Ok(()) => return SlotLock::claim(path),
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(err) => return Err(lock_error(&err)),
      }
      match inspect(&path, stale_after) {
        Seen::Retry if attempt == 0 => {}
        Seen::Stale(_, _, seen) if attempt == 0 && identity_of(&path) == Some(seen) => {
          let _gone = std::fs::remove_dir_all(&path);
        }
        Seen::Held(pid, since) | Seen::Stale(pid, since, _) => return Err(locked(pid, since)),
        Seen::Retry => return Err(locked(None, None)),
      }
    }
    Err(locked(None, None))
  }

  fn claim(path: PathBuf) -> Result<SlotLock> {
    let identity = std::fs::metadata(&path)
      .map(|meta| (meta.dev(), meta.ino()))
      .map_err(|err| claim_error(&err))?;
    let lock = SlotLock {
      path,
      identity: Some(identity),
    };
    std::fs::write(lock.path.join("pid"), format!("{}\n", std::process::id()))
      .and_then(|()| std::fs::write(lock.path.join("since"), format!("{}\n", now_secs())))
      .map_err(|err| claim_error(&err))?;
    Ok(lock)
  }

  fn release(&mut self) {
    let Some(identity) = self.identity.take() else {
      return;
    };
    if identity_of(&self.path) == Some(identity) {
      let _gone = std::fs::remove_dir_all(&self.path);
    }
  }
}

impl Drop for SlotLock {
  fn drop(&mut self) {
    self.release();
  }
}

fn identity_of(path: &Path) -> Option<(u64, u64)> {
  std::fs::metadata(path)
    .ok()
    .map(|meta| (meta.dev(), meta.ino()))
}

/// The lock as found. A stale lock carries the identity of the directory that
/// was judged, so a lock another controller made since is never removed.
fn inspect(path: &Path, stale_after: Option<u64>) -> Seen {
  let Some(seen) = identity_of(path) else {
    return Seen::Retry;
  };
  let pid = read_number(&path.join("pid"));
  let since = read_number(&path.join("since"));
  let stale = if let (Some(pid), Some(since)) = (pid, since) {
    let dead = !pid_alive(i64::try_from(pid).unwrap_or(i64::MAX)).unwrap_or(true);
    dead || stale_after.is_some_and(|limit| now_secs().saturating_sub(since) > limit)
  } else {
    let Ok(modified) = std::fs::metadata(path).and_then(|meta| meta.modified()) else {
      return Seen::Retry;
    };
    SystemTime::now()
      .duration_since(modified)
      .is_ok_and(|age| age >= Duration::from_secs(1))
  };
  if stale {
    Seen::Stale(pid, since, seen)
  } else {
    Seen::Held(pid, since)
  }
}

#[cfg(test)]
#[path = "tests/lock_test.rs"]
mod tests;
