use crate::model::{Issue, Seen};

#[test]
fn a_blank_issue_has_no_stage_and_its_key_as_agent() {
  let issue = Issue::blank("o-r-7", "o-r-1", "/state", 42);
  assert_eq!(
    (issue.key.as_str(), issue.epic.as_str()),
    ("o-r-7", "o-r-1")
  );
  assert_eq!(issue.agent, "o-r-7");
  assert!(issue.stage.is_empty() && issue.graph_status.is_empty());
  assert_eq!(issue.agent_status, "unknown");
  assert_eq!(issue.phase_at_ms, 42);
  assert_eq!(issue.tracker_seen, Seen::Open);
  assert!(issue.pending.is_none() && issue.hold.is_none());
}
