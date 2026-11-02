//! Durable routing identity for a controller submission.
use crate::relay::config;
use crate::relay::durable;
use std::path::Path;
use std::time::Duration;
use toolu_runtime::env::Env;
use toolu_state::lock::{LockOptions, with_lock_strict};

fn path(root: &Path, key: &str) -> Result<std::path::PathBuf, String> {
  if !crate::relay::store::safe(key) {
    return Err("invalid key".into());
  }
  Ok(
    root
      .join("outbox")
      .join("endpoints")
      .join(format!("{key}.json")),
  )
}

/// Resolve a saved endpoint, using current configuration for unknown tasks.
pub(super) fn get(env: &Env, key: &str) -> Result<String, String> {
  let path = path(&config::root(env), key)?;
  match std::fs::read(path) {
    Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| format!("invalid endpoint: {e}")),
    Err(e) if e.kind() == std::io::ErrorKind::NotFound => current(env),
    Err(e) => Err(e.to_string()),
  }
}

fn current(env: &Env) -> Result<String, String> {
  env
    .get("TOOLU_RELAY_TARGET")
    .map(str::to_owned)
    .or(config::load(&config::root(env))?.relay)
    .ok_or_else(|| "relay is not configured".into())
}

/// Atomically persist the first endpoint before any network delivery.
pub(super) fn pin(env: &Env, key: &str) -> Result<String, String> {
  let path = path(&config::root(env), key)?;
  let dir = path.parent().ok_or("invalid endpoint path")?;
  durable::ensure_dir(dir)?;
  let options = LockOptions {
    stale: Duration::MAX,
    ..LockOptions::default()
  };
  with_lock_strict(&path, options, || {
    if path.exists() {
      return get(env, key);
    }
    let endpoint = current(env)?;
    let data = serde_json::to_vec(&endpoint).map_err(|e| e.to_string())?;
    durable::write(&path, &data)?;
    Ok(endpoint)
  })?
}

#[cfg(test)]
#[path = "tests/endpoint_test.rs"]
mod tests;
