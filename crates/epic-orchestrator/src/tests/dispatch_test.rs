use serde_json::json;

use super::report_from;

#[test]
fn a_report_request_copies_its_phase() {
  let report = report_from(&json!({
    "key": "a",
    "epic": "one",
    "state_dir": "/tmp",
    "phase": "ready",
  }));
  assert_eq!(report.key, "a");
  assert_eq!(report.phase, "ready");
  assert_eq!(report.epic, "one");
}
