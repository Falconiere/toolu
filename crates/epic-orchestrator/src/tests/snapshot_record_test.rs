use serde_json::json;

use super::overlay_record;
use crate::model::Issue;

#[test]
fn the_records_engine_fields_overlay_the_issue() {
  let mut issue = Issue::blank("k", "e", "/state", 0);
  let record = json!({
    "hold": {"kind": "coupling", "with": "j"},
    "coupling_released": true,
    "relaunch": "lost",
    "deferred": "launch",
    "worktree": "/trees/k",
    "kind": "codex"
  });
  overlay_record(&mut issue, &record);
  assert_eq!(issue.hold, Some(json!({"kind": "coupling", "with": "j"})));
  assert!(issue.coupling_released);
  assert_eq!(issue.relaunch.as_deref(), Some("lost"));
  assert_eq!(issue.deferred.as_deref(), Some("launch"));
  assert_eq!(issue.worktree.as_deref(), Some("/trees/k"));
  assert_eq!(issue.kind, "codex");
}

#[test]
fn a_null_hold_and_missing_fields_leave_the_issue_blank() {
  let mut issue = Issue::blank("k", "e", "/state", 0);
  overlay_record(&mut issue, &json!({"hold": null}));
  assert!(issue.hold.is_none() && issue.relaunch.is_none() && issue.worktree.is_none());
  assert!(!issue.coupling_released);
  assert_eq!(issue.kind, "");
}
