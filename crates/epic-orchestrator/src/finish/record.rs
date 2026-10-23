//! The strict issue record a finish trusts (`finish-state.ts`), its removal
//! intent, and the document a finish prints.

use std::path::Path;

use serde_json::{Value, json};

use crate::launch::record::{Record, text};
use crate::workload::OwnedWorkload;

const STAGES: [&str; 9] = [
  "starting",
  "running",
  "awaiting_merge",
  "replacing",
  "cleaning",
  "uncertain",
  "cleanup-incomplete",
  "merged",
  "abandoned",
];

/// The durable boundary written before a worktree is removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RemovalIntent {
  /// The workspace being removed.
  pub workspace_id: String,
  /// The worktree being removed.
  pub worktree: String,
  /// The snapshot ref taken before removal.
  pub wip_ref: Option<String>,
  /// Uncommitted files at removal.
  pub leftover_files: Vec<String>,
  /// The workload that had to be gone.
  pub workload: Vec<OwnedWorkload>,
}

impl RemovalIntent {
  /// The record value, stamped `prepared_at`.
  pub(crate) fn to_value(&self, prepared_at: &str) -> Value {
    let workload: Vec<Value> = self
      .workload
      .iter()
      .map(|owned| json!({"pid": owned.pid, "group": owned.group, "started": owned.started}))
      .collect();
    json!({
      "version": 1,
      "workspace_id": self.workspace_id,
      "worktree": self.worktree,
      "prepared_at": prepared_at,
      "wip_ref": self.wip_ref,
      "leftover_files": self.leftover_files,
      "workload": workload,
    })
  }
}

fn owned(entry: &Value) -> Option<OwnedWorkload> {
  let positive = |key: &str| {
    entry
      .get(key)
      .and_then(Value::as_u64)
      .filter(|n| *n > 0)
      .and_then(|n| u32::try_from(n).ok())
  };
  let started = entry
    .get("started")
    .and_then(Value::as_str)
    .filter(|s| !s.is_empty())?;
  Some(OwnedWorkload {
    pid: positive("pid")?,
    group: positive("group")?,
    started: started.to_owned(),
  })
}

fn strings(value: Option<&Value>) -> Option<Vec<String>> {
  value?
    .as_array()?
    .iter()
    .map(|entry| entry.as_str().map(str::to_owned))
    .collect()
}

/// The record's removal intent, if it has one.
///
/// # Errors
/// The intent is malformed.
pub(crate) fn removal_intent(record: &Record) -> Result<Option<RemovalIntent>, String> {
  let Some(value) = record.get("removal_intent") else {
    return Ok(None);
  };
  let invalid = || "invalid removal intent".to_owned();
  let field = |key: &str| value.get(key).and_then(Value::as_str);
  let prepared = field("prepared_at").and_then(toolu_state::time::parse_iso);
  let wip_ref = match value.get("wip_ref") {
    Some(Value::Null) => None,
    Some(Value::String(reference)) => Some(reference.clone()),
    _ => return Err(invalid()),
  };
  let workload: Option<Vec<OwnedWorkload>> = value
    .get("workload")
    .and_then(Value::as_array)
    .and_then(|items| items.iter().map(owned).collect());
  let (Some(workspace_id), Some(worktree), Some(_), Some(leftover_files), Some(workload)) = (
    field("workspace_id").filter(|id| !id.is_empty()),
    field("worktree").filter(|path| Path::new(path).is_absolute()),
    prepared,
    strings(value.get("leftover_files")),
    workload,
  ) else {
    return Err(invalid());
  };
  if value.get("version").and_then(Value::as_u64) != Some(1) {
    return Err(invalid());
  }
  Ok(Some(RemovalIntent {
    workspace_id: workspace_id.to_owned(),
    worktree: worktree.to_owned(),
    wip_ref,
    leftover_files,
    workload,
  }))
}

/// The record of `key` at `path`, validated for a finish.
///
/// # Errors
/// The record is missing, not an object, or malformed.
pub(crate) fn read_issue_record(path: &Path, key: &str) -> Result<Record, String> {
  let Ok(Value::Object(record)) = crate::disk::read_value(path) else {
    return Err(format!(
      "invalid or missing issue record {}",
      path.display()
    ));
  };
  let invalid = |detail: &str| format!("invalid issue record {}: {detail}", path.display());
  let started = !never_started(&record);
  let required = if started {
    &["key", "repo", "branch", "checkout", "worktree", "stage"][..]
  } else {
    &["key", "repo", "branch", "stage"][..]
  };
  for field in required {
    if text(&record, field).is_none_or(str::is_empty) {
      return Err(invalid(field));
    }
  }
  if text(&record, "key") != Some(key) {
    return Err(invalid("key mismatch"));
  }
  if !text(&record, "stage").is_some_and(|stage| STAGES.contains(&stage)) {
    return Err(invalid("stage"));
  }
  let absolute =
    |field: &str| text(&record, field).is_some_and(|path| Path::new(path).is_absolute());
  let relative = |field: &str| record.contains_key(field) && !absolute(field);
  if relative("checkout") || relative("worktree") {
    return Err(invalid("paths"));
  }
  let (root, token) = (record.get("resource_root"), record.get("lease_token"));
  if root.is_some() != token.is_some() {
    return Err(invalid("partial resource ownership"));
  }
  let bad_root = root.is_some_and(|root| {
    !root
      .as_str()
      .is_some_and(|path| Path::new(path).is_absolute())
  });
  let bad_token = token.is_some_and(|token| token.as_str().is_none_or(str::is_empty));
  if bad_root || bad_token {
    return Err(invalid("resource ownership"));
  }
  removal_intent(&record).map_err(|_err| invalid("removal intent"))?;
  Ok(record)
}

/// Whether the attempt died before it made anything a teardown must remove:
/// no worktree, workspace, agent, pane, session or removal intent.
pub(crate) fn never_started(record: &Record) -> bool {
  let empty = |field: &str| record.get(field).is_none_or(|v| v.is_null() || v == "");
  [
    "worktree",
    "workspace_id",
    "agent",
    "pane_id",
    "session_id",
    "removal_intent",
  ]
  .iter()
  .all(|field| empty(field))
}

/// Whether a terminal record proves its cleanup finished.
pub(crate) fn terminal_cleanup_complete(record: &Record) -> bool {
  let removed = record.get("worktree_removed") == Some(&Value::Bool(true))
    && !text(record, "worktree").is_some_and(|path| Path::new(path).exists());
  removed
    && (text(record, "stage") == Some("abandoned")
      || record.get("branch_deleted") == Some(&Value::Bool(true)))
}

/// The leftover files the record lists.
pub(crate) fn leftover_files(record: &Record) -> Vec<String> {
  record
    .get("leftover_files")
    .and_then(Value::as_array)
    .map(|items| {
      items
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
    })
    .unwrap_or_default()
}

/// What a finish prints: key, stage, removal and branch evidence.
pub(crate) fn finished(record: &Record) -> Value {
  json!({
    "key": text(record, "key"),
    "stage": text(record, "stage"),
    "worktree_removed": record.get("worktree_removed") == Some(&Value::Bool(true)),
    "branch_deleted": record.get("branch_deleted") == Some(&Value::Bool(true)),
    "wip_ref": text(record, "wip_ref"),
    "leftover_files": leftover_files(record),
  })
}

#[cfg(test)]
#[path = "tests/record_test.rs"]
mod tests;
