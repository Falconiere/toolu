//! What the sandbox keeps on the host, under `$XDG_STATE_HOME/toolu/sandbox` (or
//! `~/.local/state/toolu/sandbox`): the generated Lima file, the logs of the long
//! steps, the stamp a finished `setup` writes, and the lock that keeps two
//! sandbox commands from starting or changing the same VM at once.

use std::fs::{File, TryLockError};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use toolu_protocol::exit::Exit;
use toolu_runtime::atomic::write_atomic;
use toolu_runtime::env::Env;

use crate::step::Failure;

/// How long a command waits for the VM's lock before it reports the VM busy: long
/// enough for the moment another command takes to let go (a `flock` is also held
/// by a process that forked in another thread until that child execs), short
/// enough to fail fast while a boot or a setup is really running.
const LOCK_WAIT: Duration = Duration::from_secs(2);
/// How often a waiting command looks at the lock again.
const LOCK_POLL: Duration = Duration::from_millis(50);

/// The files of one VM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct State {
  dir: PathBuf,
  name: String,
  wait: Duration,
}

impl State {
  /// The state of VM `name`.
  pub(crate) fn new(env: &Env, name: &str) -> State {
    let base = env
      .get("XDG_STATE_HOME")
      .map_or_else(|| env.home().join(".local/state"), PathBuf::from);
    State {
      dir: base.join("toolu/sandbox"),
      name: name.to_owned(),
      wait: LOCK_WAIT,
    }
  }

  /// The same state, waiting up to `wait` for the VM's lock before reporting it busy.
  pub(crate) fn waiting(mut self, wait: Duration) -> State {
    self.wait = wait;
    self
  }

  /// The generated Lima file.
  pub(crate) fn yaml(&self) -> PathBuf {
    self.dir.join(format!("{}.yaml", self.name))
  }

  /// The log of `step` (`start`, `setup`).
  pub(crate) fn log(&self, step: &str) -> PathBuf {
    self.dir.join(format!("{}-{step}.log", self.name))
  }

  /// The stamp a finished `setup` writes.
  pub(crate) fn stamp(&self) -> PathBuf {
    self.dir.join(format!("{}.json", self.name))
  }

  /// Make the state directory.
  ///
  /// # Errors
  /// Exit 1 naming the directory when it cannot be made.
  pub(crate) fn create(&self) -> Result<(), Failure> {
    std::fs::create_dir_all(&self.dir).map_err(|err| {
      Failure::new(
        Exit::Failure,
        format!("cannot create {}: {err}", self.dir.display()),
      )
    })
  }

  /// The VM's lock, held until the returned file drops, waiting up to the state's
  /// wait for a holder to let go.
  ///
  /// # Errors
  /// Exit 75 when another sandbox command still holds it; 1 when it cannot be opened.
  pub(crate) fn lock(&self) -> Result<File, Failure> {
    self.create()?;
    let path = self.dir.join(format!("{}.lock", self.name));
    let file = File::create(&path).map_err(|err| {
      Failure::new(
        Exit::Failure,
        format!("cannot open {}: {err}", path.display()),
      )
    })?;
    let deadline = Instant::now() + self.wait;
    loop {
      match file.try_lock() {
        Ok(()) => return Ok(file),
        Err(TryLockError::WouldBlock) if Instant::now() < deadline => {
          std::thread::sleep(LOCK_POLL);
        }
        Err(TryLockError::WouldBlock) => {
          return Err(Failure::new(
            Exit::TempFail,
            format!(
              "another toolu sandbox command is changing {}; try again when it ends",
              self.name
            ),
          ));
        }
        Err(TryLockError::Error(err)) => {
          return Err(Failure::new(
            Exit::Failure,
            format!("cannot lock {}: {err}", path.display()),
          ));
        }
      }
    }
  }

  /// Forget that the VM was provisioned, before a setup that may not finish.
  pub(crate) fn clear_stamp(&self) {
    let _gone = std::fs::remove_file(self.stamp());
  }

  /// Remove the generated file and the stamp of a deleted VM.
  pub(crate) fn forget(&self) {
    for path in [self.yaml(), self.stamp()] {
      let _gone = std::fs::remove_file(path);
    }
  }
}

/// Write `text` to `path` atomically.
///
/// # Errors
/// Exit 1 naming the file.
pub(crate) fn write(path: &Path, text: &str) -> Result<(), Failure> {
  if write_atomic(path, text) {
    Ok(())
  } else {
    Err(Failure::new(
      Exit::Failure,
      format!("cannot write {}", path.display()),
    ))
  }
}

#[cfg(test)]
#[path = "tests/state_test.rs"]
mod tests;
