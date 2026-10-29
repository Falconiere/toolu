//! Persist reconnect deadlines independently of the legacy watcher.

use std::path::Path;
use std::time::{Duration, SystemTime};

use serde_json::json;

#[derive(Default)]
pub(super) struct Backoff {
  pub(super) failures: u32,
  pub(super) retry_at: u64,
}

fn now() -> u64 {
  toolu_state::time::epoch_millis(SystemTime::now())
}

impl Backoff {
  pub(super) fn load(path: &Path) -> Self {
    let value = crate::disk::read_value(path).unwrap_or_default();
    Self {
      failures: value
        .get("failures")
        .and_then(serde_json::Value::as_u64)
        .and_then(|n| u32::try_from(n).ok())
        .unwrap_or(0),
      retry_at: value
        .get("retryAt")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0),
    }
  }

  pub(super) fn save(&self, path: &Path) {
    let _saved = crate::disk::write_value(
      path,
      &json!({"failures": self.failures, "retryAt": self.retry_at}),
    );
  }

  pub(super) fn failed(&mut self, path: &Path) {
    self.failures = self.failures.saturating_add(1);
    let delay = 1000_u64
      .saturating_mul(1_u64 << self.failures.saturating_sub(1).min(5))
      .min(30_000);
    self.retry_at = now().saturating_add(delay);
    self.save(path);
  }

  pub(super) fn remaining(&self) -> Duration {
    Duration::from_millis(self.retry_at.saturating_sub(now()).min(30_000))
  }
}

#[cfg(test)]
#[path = "tests/backoff_test.rs"]
mod tests;
