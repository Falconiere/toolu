//! The listener's status credentials, re-read when `secrets.json` changes so
//! `toolu epic token new` takes effect without a restart.

use std::hash::{DefaultHasher, Hash as _, Hasher as _};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use toolu_runtime::config::secrets::{self, Secrets};
use toolu_runtime::host::roots::Roots;

/// How often the file's modification time is checked.
const CHECK_EVERY: Duration = Duration::from_secs(1);

/// Secrets the listener serves with, reloaded from `roots` when it is set.
pub(crate) struct LiveSecrets {
  roots: Option<Roots>,
  state: Mutex<State>,
}

struct State {
  secrets: Arc<Secrets>,
  /// A fingerprint of the file's bytes, so rotations within one mtime tick count.
  stamp: Option<u64>,
  checked: Instant,
}

impl LiveSecrets {
  /// Serve `secrets`; with `roots`, follow later changes to its `secrets.json`.
  pub(crate) fn new(secrets: Secrets, roots: Option<Roots>) -> Self {
    Self {
      roots,
      state: Mutex::new(State {
        secrets: Arc::new(secrets),
        // No stamp: `secrets` was read before the listener started, so the
        // first check re-reads any file a rotation in between replaced.
        stamp: None,
        checked: Instant::now(),
      }),
    }
  }

  /// The current secrets. A file whose bytes changed is re-read at most once per second;
  /// a read that fails keeps the last good secrets and is retried.
  pub(crate) fn current(&self) -> Arc<Secrets> {
    let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(roots) = &self.roots
      && state.checked.elapsed() >= CHECK_EVERY
    {
      state.checked = Instant::now();
      let now = stamp(roots);
      if now != state.stamp
        && let Ok(fresh) = secrets::load(roots)
      {
        state.secrets = Arc::new(fresh);
        state.stamp = now;
      }
    }
    Arc::clone(&state.secrets)
  }
}

fn stamp(roots: &Roots) -> Option<u64> {
  let bytes = std::fs::read(secrets::path(roots)).ok()?;
  let mut hasher = DefaultHasher::new();
  bytes.hash(&mut hasher);
  Some(hasher.finish())
}

#[cfg(test)]
#[path = "tests/http_secrets_test.rs"]
mod tests;
