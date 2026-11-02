//! Durable metadata and immutable input/result blobs.

use super::{Ledger, Store, safe};
use crate::relay::durable::ensure_dir;
pub(crate) use crate::relay::durable::write;
use std::io::ErrorKind;
use std::time::Duration;
use toolu_state::lock::{LockOptions, with_lock_strict};

impl Store {
  pub(super) fn read(&self) -> Result<Ledger, String> {
    match std::fs::read(self.root.join("relay.json")) {
      Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| e.to_string()),
      Err(e) if e.kind() == ErrorKind::NotFound => Ok(Ledger::default()),
      Err(e) => Err(e.to_string()),
    }
  }

  pub(super) fn edit<T>(
    &self,
    change: impl FnOnce(&mut Ledger) -> Result<T, String>,
  ) -> Result<T, String> {
    ensure_dir(&self.root)?;
    let path = self.root.join("relay.json");
    let options = LockOptions {
      timeout: Duration::from_secs(5),
      stale: Duration::MAX,
    };
    with_lock_strict(&path, options, || {
      let mut ledger = self.read()?;
      let before = serde_json::to_vec(&ledger).map_err(|e| e.to_string())?;
      let answer = change(&mut ledger)?;
      let after = serde_json::to_vec(&ledger).map_err(|e| e.to_string())?;
      if before != after {
        write(&path, &after)?;
      }
      Ok(answer)
    })?
  }

  pub(super) fn read_blob(&self, key: &str, name: &str) -> Result<Vec<u8>, String> {
    if !safe(key) {
      return Err("invalid task key".into());
    }
    std::fs::read(self.root.join("tasks").join(key).join(name)).map_err(|e| e.to_string())
  }

  pub(super) fn blob(&self, key: &str, name: &str, bytes: &[u8]) -> Result<(), String> {
    if !safe(key) {
      return Err("invalid task key".into());
    }
    let dir = self.root.join("tasks").join(key);
    ensure_dir(&dir)?;
    let path = dir.join(name);
    match std::fs::read(&path) {
      Ok(saved) if saved == bytes => Ok(()),
      Ok(_) => Err("durable blob conflicts with existing contents".into()),
      Err(e) if e.kind() == ErrorKind::NotFound => write(&path, bytes),
      Err(e) => Err(e.to_string()),
    }
  }
}

#[cfg(test)]
#[path = "tests/disk_test.rs"]
mod tests;
