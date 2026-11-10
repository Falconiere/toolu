//! The engine's own fields of an issue record, overlaid on a loaded issue.

use serde_json::Value;

/// The engine's fields of an issue record: its hold, release, waiting
/// relaunch, tracker sighting, worktree and host.
pub(crate) fn overlay_record(issue: &mut crate::model::Issue, record: &Value) {
  overlay_target(issue, record);
  issue.hold = record.get("hold").filter(|hold| !hold.is_null()).cloned();
  issue.coupling_released = record.get("coupling_released") == Some(&Value::Bool(true));
  issue.relaunch = record
    .get("relaunch")
    .and_then(Value::as_str)
    .map(str::to_owned);
  issue.deferred = record
    .get("deferred")
    .and_then(Value::as_str)
    .map(str::to_owned);
  issue.tracker_seen = crate::merge_bridge::seen_from(record);
  crate::queue_fields::read(issue, record);
  if let Some(tree) = record.get("worktree").and_then(Value::as_str) {
    issue.worktree = Some(tree.to_owned());
  }
  if let Some(kind) = record.get("kind").and_then(Value::as_str) {
    kind.clone_into(&mut issue.kind);
  }
}

/// Project local launch identity into the engine snapshot.
pub(crate) fn overlay_target(issue: &mut crate::model::Issue, record: &Value) {
  if let Some(agent) = record.get("agent").and_then(Value::as_str) {
    agent.clone_into(&mut issue.agent);
  }
  if let Some(pane) = record.get("pane_id").and_then(Value::as_str) {
    issue.pane = Some(pane.to_owned());
  }
  if let Some(worktree) = record.get("worktree").and_then(Value::as_str) {
    issue.worktree = Some(worktree.to_owned());
  }
}

#[cfg(test)]
#[path = "tests/snapshot_record_test.rs"]
mod tests;
