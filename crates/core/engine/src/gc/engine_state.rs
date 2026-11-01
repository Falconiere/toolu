//! The engine's own leftovers: job logs past retention, and issue records that
//! still say a worker is busy although its worktree is gone. Logs are pruned
//! through the catalog's retention; records are only reported with their fix.

use std::path::Path;

use serde_json::{Value, json};

use super::Request;
use super::repos::IssueRecord;
use crate::resources::job_logs;

/// Stages that mean "a worker holds this" while the worktree should exist.
const BUSY: [&str; 5] = [
  "starting",
  "running",
  "awaiting_merge",
  "cleaning",
  "cleanup-incomplete",
];

/// Prune job logs with `apply`, and list the stale records.
pub(crate) fn sweep(request: &Request, records: &[IssueRecord]) -> Value {
  let logs = request.resource_root.join("jobs").join("logs");
  let before = count_logs(&logs);
  let pruned = if request.apply {
    job_logs::list(&request.resource_root, "").err()
  } else {
    None
  };
  let after = count_logs(&logs);
  json!({
    "job_logs": after,
    "job_logs_removed": before.saturating_sub(after),
    "job_logs_error": pruned,
    "stale_records": stale(records),
  })
}

fn count_logs(dir: &Path) -> usize {
  std::fs::read_dir(dir).map_or(0, |entries| {
    entries
      .flatten()
      .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "log"))
      .count()
  })
}

/// Records at a busy stage whose worktree no longer exists, with the command that settles them.
pub(crate) fn stale(records: &[IssueRecord]) -> Vec<Value> {
  records
    .iter()
    .filter(|record| BUSY.contains(&record.stage.as_str()))
    .filter(|record| record.worktree.as_ref().is_some_and(|path| !path.exists()))
    .map(|record| {
      json!({
        "state_dir": record.state_dir,
        "key": record.key,
        "stage": record.stage,
        "fix": format!(
          "toolu epic finish --abandon {} {}",
          record.state_dir.display(),
          record.key
        ),
      })
    })
    .collect()
}

#[cfg(test)]
#[path = "tests/engine_state_test.rs"]
mod tests;
