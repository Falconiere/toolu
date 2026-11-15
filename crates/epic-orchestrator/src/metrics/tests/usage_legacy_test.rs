use serde_json::json;
use toolu_runtime::env::Env;

use crate::launch::record::write_record;
use crate::metrics::{usage, usage_report};

#[test]
fn usage_legacy_uncertain_launch_without_session_remains_visible() {
  let tmp = tempfile::tempdir().unwrap();
  let env = Env::from_pairs([("HOME", tmp.path().to_str().unwrap())]);
  let mut record = json!({
    "key": "comemory-255", "kind": "codex", "worktree": "/w/b",
    "stage": "uncertain", "attempt_started_at": "2026-10-10T00:00:00Z"
  })
  .as_object()
  .unwrap()
  .clone();
  usage::enroll(&mut record, &env).unwrap();
  assert_eq!(record["usage_history"]["legacy_gap"], true);
  let issues = tmp.path().join("issues");
  std::fs::create_dir_all(&issues).unwrap();
  write_record(&issues.join("comemory-255.json"), &record).unwrap();
  let report = usage_report::report(tmp.path(), &env).unwrap();
  assert_eq!(report["coverage"], "unavailable");
  let attempts = report["attempts"].as_array().unwrap();
  assert_eq!(attempts.len(), 1);
  assert!(attempts[0]["session"].is_null());
  assert_eq!(attempts[0]["row"]["status"], "unavailable");
  assert!(
    attempts[0]["row"]["reason"]
      .as_str()
      .is_some_and(|s| !s.is_empty())
  );
  assert!(!report["reasons"].as_array().unwrap().is_empty());

  let mut fresh = json!({"backend":"herdr"}).as_object().unwrap().clone();
  usage::enroll(&mut fresh, &env).unwrap();
  assert_eq!(fresh["usage_history"]["legacy_gap"], false);
  assert!(
    fresh["usage_history"]["attempts"]
      .as_array()
      .unwrap()
      .is_empty()
  );
}
