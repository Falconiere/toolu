use serde_json::json;

use super::{bot, checks, ci_status, memory};

#[test]
fn check_runs_and_status_contexts_map_to_pass_pending_fail() {
  let pr = json!({"statusCheckRollup": [
    {"__typename": "CheckRun", "name": "a", "status": "COMPLETED", "conclusion": "SKIPPED", "detailsUrl": "u"},
    {"__typename": "CheckRun", "name": "b", "status": "IN_PROGRESS"},
    {"__typename": "StatusContext", "context": "c", "state": "EXPECTED", "targetUrl": "t"},
    {"__typename": "StatusContext", "context": "d", "state": "ERROR"},
    {"__typename": "CheckRun", "status": "COMPLETED", "conclusion": "TIMED_OUT"},
  ]});
  let rows = checks(&pr);
  let statuses: Vec<&str> = rows
    .iter()
    .map(|row| row["status"].as_str().unwrap())
    .collect();
  assert_eq!(statuses, ["pass", "pending", "pending", "fail", "fail"]);
  assert_eq!(rows[2]["name"], "c");
  assert_eq!(rows[2]["url"], "t");
  assert_eq!(rows[4]["name"], "unknown");
  assert_eq!(ci_status(&rows), "fail");
  assert_eq!(ci_status(&rows[..3]), "pending");
  assert_eq!(ci_status(&rows[..1]), "pass");
  assert_eq!(ci_status(&[]), "pending");
}

#[test]
fn the_bot_and_memory_read_their_defaults() {
  let empty = bot(&json!({}), None);
  assert_eq!(empty.state, "absent");
  assert_eq!(empty.degraded, Some("review_absent"));
  assert!(!empty.same_run);
  let snapshot = json!({"bot": {"comment": {"id": 5, "updatedAt": "t"}, "verdict": {"state": "complete", "verdict": "approved", "is_review_comment": true, "findings": [{"key": "k"}]}}});
  let previous = json!({"pr": {"botCommentId": 5, "botCommentUpdatedAt": "t"}});
  let read = bot(&snapshot, Some(&previous));
  assert!(read.same_run);
  assert_eq!(read.keys, [json!("k")]);
  assert_eq!(read.degraded, None);
  let fresh = memory(None);
  assert_eq!(
    fresh.actions,
    json!({"replied": {}, "resolved": {}, "flagged": {}})
  );
  assert!(fresh.fixing.is_none());
  let busy = memory(Some(
    &json!({"fixer": {"status": "blocked", "items": ["a", 3]}, "totalTicks": 4}),
  ));
  assert!(busy.owns(&json!({"id": 3})));
  assert!(!busy.owns(&json!({"id": "b"})));
  assert_eq!(busy.ticks, 4);
}
