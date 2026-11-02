use super::{completed, matches_report};
use crate::relay::durable::create as durable_marker;

#[test]
fn interrupted_recipe_never_replays_and_completed_recipe_must_match() {
  let dir = tempfile::tempdir().unwrap();
  let intent = dir.path().join("intent");
  let done = dir.path().join("done");
  assert!(!completed(&intent, &done, b"command").unwrap());
  durable_marker(&intent, b"command").unwrap();
  assert!(completed(&intent, &done, b"command").is_err());
  durable_marker(&done, b"done").unwrap();
  assert!(completed(&intent, &done, b"command").unwrap());
  assert!(completed(&intent, &done, b"other").is_err());
}

#[test]
fn report_requires_the_reserved_attempt_and_input_commit() {
  let report = serde_json::json!({"attempt": "a", "base": "b"});
  assert!(matches_report(&report, "a", "b"));
  assert!(!matches_report(&report, "other", "b"));
  assert!(!matches_report(&report, "a", "other"));
}
