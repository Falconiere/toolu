use super::*;
use crate::relay::store::tests::{config, submission};

#[test]
fn failed_outcome_requires_current_attempt_and_releases_capacity() {
  let dir = tempfile::tempdir().unwrap();
  let store = Store::new(dir.path());
  let cfg = config(&[("worker", 1, false, &["codex"])]);
  for key in ["one", "two"] {
    store.submit(submission(key), b"input").unwrap();
  }
  let first = store.reserve("one", &cfg, 0).unwrap().unwrap();
  assert!(store.fail("one", "stale", "failed").is_err());
  let failed = store
    .fail(
      "one",
      first.attempt.as_deref().unwrap(),
      "could not complete",
    )
    .unwrap();
  assert_eq!(failed.phase, "failed");
  assert_eq!(store.cancel("one").unwrap().phase, "failed");
  assert!(store.reserve("two", &cfg, 0).unwrap().is_some());
}
