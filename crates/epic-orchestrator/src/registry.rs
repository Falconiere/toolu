//! Serialize registry read-modify-write operations across registration and cleanup.

use std::time::Duration;

use serde_json::{Value, json};
use toolu_engine::resources::lock::acquire_lock;

use crate::paths::Paths;

/// Run a registry update under the shared, crash-reclaimable lock.
pub(crate) fn update<T>(paths: &Paths, f: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
  let deadline = std::time::Instant::now() + Duration::from_secs(5);
  let held = loop {
    if let Some(held) = acquire_lock(&paths.root.join("registry.lock"), Duration::ZERO)? {
      break held;
    }
    if std::time::Instant::now() >= deadline {
      return Err("registry busy; retry later".to_owned());
    }
    std::thread::sleep(Duration::from_millis(25));
  };
  let result = f();
  held.release();
  result
}

/// Read a registry without treating damaged contents as an empty registry.
pub(crate) fn read(paths: &Paths) -> Result<Value, String> {
  if matches!(paths.registry().try_exists(), Ok(false)) {
    return Ok(json!({"version": 1, "epics": []}));
  }
  let value = crate::disk::read_value(&paths.registry())?;
  if !value.get("epics").is_some_and(Value::is_array) {
    return Err("invalid epic registry: epics must be an array".to_owned());
  }
  Ok(value)
}

#[cfg(test)]
#[path = "tests/registry_test.rs"]
mod tests;
