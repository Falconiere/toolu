//! Serialized, fsynced peer state, separate from delegation and each authority.

use super::types::{Error, Ledger, safe};
use crate::relay::durable;
use std::path::Path;
use std::time::Duration;
use toolu_state::lock::{LockOptions, with_lock_strict};

/// Change peer state under a strict lock and persist it before returning.
pub(super) fn edit<T>(
  root: &Path,
  generation: &str,
  change: impl FnOnce(&mut Ledger) -> Result<T, Error>,
) -> Result<T, Error> {
  if !safe(generation) {
    return Err(Error::new("invalid", "invalid authority generation"));
  }
  let dir = root.join("peers").join("relay").join(generation);
  durable::ensure_dir(&dir).map_err(Error::storage)?;
  let path = dir.join("ledger.json");
  let options = LockOptions {
    stale: Duration::MAX,
    ..LockOptions::default()
  };
  with_lock_strict(&path, options, || {
    let mut ledger = read(&path)?;
    let before = serde_json::to_vec(&ledger).map_err(|e| Error::storage(e.to_string()))?;
    let result = change(&mut ledger)?;
    let after = serde_json::to_vec(&ledger).map_err(|e| Error::storage(e.to_string()))?;
    if before != after {
      durable::write(&path, &after).map_err(Error::storage)?;
    }
    Ok(result)
  })
  .map_err(Error::storage)?
}

fn read(path: &Path) -> Result<Ledger, Error> {
  let bytes = match std::fs::read(path) {
    Ok(bytes) => bytes,
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Ledger::default()),
    Err(error) => return Err(Error::storage(error.to_string())),
  };
  let ledger: Ledger = serde_json::from_slice(&bytes).map_err(|e| Error::storage(e.to_string()))?;
  if ledger.version != 1 {
    return Err(Error::new("invalid", "unsupported peer storage version"));
  }
  Ok(ledger)
}

#[cfg(test)]
#[path = "tests/storage_test.rs"]
mod tests;
