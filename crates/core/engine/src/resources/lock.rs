//! Exclusive ownership of the resource home (`packages/toolu-core/src/resources/lock.ts`):
//! a `state.lock` directory holding `owner.json` (`{pid, token}`). A lock is
//! prepared as `<path>.<token>.pending` and renamed into place, so it never
//! replaces an existing one. A lock whose owner died is reclaimed under
//! `<path>.reap`, which serializes reclamation; a `.reap` left by a crashed
//! reclaimer fails closed until an operator reconciles it. TypeScript and Rust
//! callers share the protocol byte for byte.

use std::io::{ErrorKind, Write as _};
use std::os::unix::fs::OpenOptionsExt as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use toolu_runtime::json::ordered::Ordered;
use toolu_runtime::process::group::pid_alive;
use toolu_state::lock::token;

use crate::ledger::jq::number;

/// The wait between two attempts on a held lock.
const RETRY: Duration = Duration::from_millis(25);
/// How long a resource update waits for the lock.
const UPDATE_TIMEOUT: Duration = Duration::from_millis(5000);

/// `processAlive(pid)`.
///
/// # Errors
/// A probe failure other than "no such process".
pub fn process_alive(pid: f64) -> Result<bool, String> {
  match whole(pid) {
    Some(pid) => pid_alive(pid),
    None => Ok(false),
  }
}

/// `value` as an integer when it is one (`Number.isInteger`) and fits an `i64`.
pub fn whole(value: f64) -> Option<i64> {
  if value.fract() != 0.0 {
    return None;
  }
  // `Display` for a whole f64 prints its digits without an exponent.
  value.to_string().parse().ok()
}

/// `readJsonFile(path, fallback)`: the parsed file, `fallback` when it does not exist.
///
/// # Errors
/// Any read failure but "not found", and text that is not JSON.
pub fn read_json_file(path: &Path, fallback: Ordered) -> Result<Ordered, String> {
  match std::fs::read(path) {
    Ok(bytes) => Ordered::parse(&String::from_utf8_lossy(&bytes))
      .map_err(|err| format!("{}: {err}", path.display())),
    Err(err) if err.kind() == ErrorKind::NotFound => Ok(fallback),
    Err(err) => Err(format!("{}: {err}", path.display())),
  }
}

/// `writeJsonAtomic(path, value)`: `JSON.stringify(value, null, 2)` and a
/// newline to an exclusive 0600 `<path>.<pid>.<uuid>.tmp`, renamed over `path`.
///
/// # Errors
/// When the directory, the temp file or the rename fails; no temp file is left.
pub fn write_json_atomic(path: &Path, value: &Ordered) -> Result<(), String> {
  if let Some(dir) = path.parent() {
    std::fs::create_dir_all(dir).map_err(|err| format!("{}: {err}", dir.display()))?;
  }
  let mut temp = path.as_os_str().to_owned();
  temp.push(format!(".{}.{}.tmp", std::process::id(), token()));
  let temp = PathBuf::from(temp);
  let written = std::fs::OpenOptions::new()
    .write(true)
    .create_new(true)
    .mode(0o600)
    .open(&temp)
    .and_then(|mut file| file.write_all(format!("{}\n", value.to_text(true)).as_bytes()))
    .and_then(|()| std::fs::rename(&temp, path));
  let _removed = std::fs::remove_file(&temp);
  written.map_err(|err| format!("{}: {err}", path.display()))
}

/// A held lock.
#[derive(Debug)]
pub struct LockHandle {
  path: PathBuf,
  token: String,
}

impl LockHandle {
  /// The lock's token, as `owner.json` holds it.
  pub fn token(&self) -> &str {
    &self.token
  }

  /// Removes the lock, but only while `owner.json` still names this handle.
  pub fn release(&self) {
    let owner =
      read_json_file(&self.path.join("owner.json"), Ordered::Null).unwrap_or(Ordered::Null);
    if matches!(owner.get("token"), Some(Ordered::String(held)) if *held == self.token) {
      let _removed = std::fs::remove_dir_all(&self.path);
    }
  }
}

fn suffixed(path: &Path, suffix: &str) -> PathBuf {
  let mut out = path.as_os_str().to_owned();
  out.push(suffix);
  PathBuf::from(out)
}

/// Whether the lock's owner is provably dead; unknown or corrupt ownership is not.
fn stale(path: &Path) -> Result<bool, String> {
  let owner = read_json_file(&path.join("owner.json"), Ordered::Null)?;
  match (owner.get("pid"), owner.get("token")) {
    (Some(Ordered::Number(pid)), Some(Ordered::String(_))) => {
      Ok(!process_alive(pid.as_f64().unwrap_or(0.0))?)
    }
    _ => Ok(false),
  }
}

/// Removes a stale lock under `<path>.reap`; another reclaimer at work is left alone.
fn reclaim(path: &Path) -> Result<(), String> {
  let reap = suffixed(path, ".reap");
  match std::fs::create_dir(&reap) {
    Ok(()) => {}
    Err(err) if err.kind() == ErrorKind::AlreadyExists => return Ok(()),
    Err(err) => return Err(format!("{}: {err}", reap.display())),
  }
  let reclaimed = stale(path).and_then(|stale| {
    if stale {
      std::fs::remove_dir_all(path).map_err(|err| format!("{}: {err}", path.display()))
    } else {
      Ok(())
    }
  });
  let _removed = std::fs::remove_dir_all(&reap);
  reclaimed
}

/// One attempt: `Ok(Some)` when taken, `Ok(None)` when held or refused.
fn attempt(path: &Path) -> Result<Option<LockHandle>, String> {
  let token = token();
  let prepared = suffixed(path, &format!(".{token}.pending"));
  let published = (|| {
    std::fs::create_dir(&prepared)?;
    let owner = Ordered::Object(vec![
      ("pid".to_owned(), number(f64::from(std::process::id()))),
      ("token".to_owned(), Ordered::String(token.clone())),
    ]);
    write_json_atomic(&prepared.join("owner.json"), &owner).map_err(std::io::Error::other)?;
    std::fs::rename(&prepared, path)
  })();
  let _removed = std::fs::remove_dir_all(&prepared);
  match published {
    Ok(()) => Ok(Some(LockHandle {
      path: path.to_path_buf(),
      token,
    })),
    Err(err)
      if matches!(
        err.kind(),
        ErrorKind::AlreadyExists | ErrorKind::DirectoryNotEmpty
      ) =>
    {
      Ok(None)
    }
    Err(err) => Err(format!("{}: {err}", path.display())),
  }
}

/// `acquireLock(path, {timeoutMs})`: the lock, or `None` once `timeout` passed
/// or a crashed reclaimer's `.reap` is in the way.
///
/// # Errors
/// A file-system failure, or a corrupt owner record met while reclaiming.
pub fn acquire_lock(path: &Path, timeout: Duration) -> Result<Option<LockHandle>, String> {
  if let Some(dir) = path.parent() {
    std::fs::create_dir_all(dir).map_err(|err| format!("{}: {err}", dir.display()))?;
  }
  let until = Instant::now() + timeout;
  loop {
    if suffixed(path, ".reap").exists() {
      return Ok(None);
    }
    if path.exists() {
      reclaim(path)?;
    }
    if !path.exists()
      && let Some(handle) = attempt(path)?
    {
      return Ok(Some(handle));
    }
    reclaim(path)?;
    if Instant::now() >= until {
      return Ok(None);
    }
    std::thread::sleep(RETRY);
  }
}

/// `withResourceLock(root, fn)`: `f` under `<root>/state.lock`.
///
/// # Errors
/// `resource state busy; retry later` past five seconds, a lock failure, or `f`'s error.
pub fn with_resource_lock<T>(
  root: &Path,
  f: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
  let lock = acquire_lock(&root.join("state.lock"), UPDATE_TIMEOUT)?
    .ok_or_else(|| "resource state busy; retry later".to_owned())?;
  let result = f();
  lock.release();
  result
}

#[cfg(test)]
#[path = "tests/lock_test.rs"]
mod tests;
