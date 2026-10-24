//! The lease half of the engine adopting a worker (#625). A worker that
//! reports is running, whatever its launch confirmed, and the engine records
//! it so. A launch whose prompt was never confirmed left its agent lease
//! `starting` or `uncertain`, and managed job admission refuses `uncertain`,
//! so the lease takes the record's `running` too. A launch in flight, a lease
//! mid-migration and every other stage are left alone.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;
use toolu_engine::resources::leases::{LeasePatch, patch_lease};
use toolu_engine::resources::lock::acquire_lock;
use toolu_engine::resources::store::{leases, read_resource_state};
use toolu_runtime::json::ordered::Ordered;

use super::record::text;

/// The resource root and lease token of `key`'s record when it is `running`.
fn running_lease(state_dir: &Path, key: &str) -> Result<Option<(PathBuf, String)>, String> {
  let path = state_dir.join("issues").join(format!("{key}.json"));
  let Value::Object(record) = crate::disk::read_value(&path)? else {
    return Ok(None);
  };
  if text(&record, "stage") != Some("running") {
    return Ok(None);
  }
  let root = text(&record, "resource_root").map(PathBuf::from);
  let token = text(&record, "lease_token").map(str::to_owned);
  Ok(root.zip(token))
}

fn string<'a>(lease: &'a Ordered, key: &str) -> Option<&'a str> {
  match lease.get(key) {
    Some(Ordered::String(value)) => Some(value),
    _ => None,
  }
}

/// Whether `token` names an agent lease a launch left before confirming its
/// prompt, and not one moving to another host.
fn unconfirmed(root: &Path, token: &str) -> Result<bool, String> {
  let state = read_resource_state(root)?;
  Ok(leases(&state).iter().any(|lease| {
    string(lease, "token") == Some(token)
      && string(lease, "type") == Some("agent")
      && matches!(string(lease, "stage"), Some("starting" | "uncertain"))
      && lease.get("pendingHost").is_none()
  }))
}

/// Give the agent lease of `key`'s `running` record the stage `running`, if
/// the launch left it `starting` or `uncertain`. `Ok(true)` when it did.
/// A held launch lock means a launch is in flight and owns the stage. The
/// lock is taken only for a lease that needs it, so a settled one never
/// contends with a launch.
///
/// # Errors
/// The record, the resource state or the lock cannot be read, or the patch
/// fails.
pub(crate) fn mirror_running(state_dir: &Path, key: &str) -> Result<bool, String> {
  let Some((root, token)) = running_lease(state_dir, key)? else {
    return Ok(false);
  };
  if !unconfirmed(&root, &token)? {
    return Ok(false);
  }
  let lock = state_dir.join("launch-locks").join(key);
  let Some(lock) = acquire_lock(&lock, Duration::ZERO)? else {
    return Ok(false);
  };
  let mirrored = unconfirmed(&root, &token).and_then(|stale| {
    if !stale {
      return Ok(false);
    }
    let patch = LeasePatch {
      stage: Some("running".to_owned()),
      ..LeasePatch::default()
    };
    patch_lease(&root, &token, &patch).map(|()| true)
  });
  lock.release();
  mirrored
}

#[cfg(test)]
#[path = "tests/adoption_test.rs"]
mod tests;
