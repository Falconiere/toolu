use serde_json::{Value, json};
use tempfile::TempDir;
use toolu_runtime::env::Env;

use super::tests::{NOW, read, recorded_165, request};
use super::{Source, tick};

#[test]
fn malformed_and_foreign_state_fail_before_replacing_bytes() {
  let dir = TempDir::new().unwrap();
  let state = dir.path().join("s.json");
  let snapshot = recorded_165();
  std::fs::write(&state, "bad JSON").unwrap();
  let failure = tick(
    &request(&state, NOW, 165),
    Source::File(&snapshot),
    &Env::default(),
  );
  assert_eq!(failure.err().unwrap().code, "state_malformed");
  assert_eq!(std::fs::read_to_string(&state).unwrap(), "bad JSON");
  std::fs::write(&state, r#"{"version":1}"#).unwrap();
  let old = tick(
    &request(&state, NOW, 165),
    Source::File(&snapshot),
    &Env::default(),
  );
  assert_eq!(old.err().unwrap().extra["version"], 1);
  std::fs::write(&state, r#"{"version":2,"repo":"Other/repo","number":165}"#).unwrap();
  let foreign = tick(
    &request(&state, NOW, 165),
    Source::File(&snapshot),
    &Env::default(),
  );
  let failure = foreign.err().unwrap();
  assert_eq!(failure.code, "slot_mismatch");
  assert_eq!(
    failure.extra["state"],
    json!({"repo": "Other/repo", "number": 165})
  );
  assert!(!dir.path().join("s.json.lock").exists());
}

#[test]
fn an_unreadable_state_or_snapshot_file_stamps_nothing() {
  let dir = TempDir::new().unwrap();
  let state = dir.path().join("s.json");
  std::fs::create_dir(&state).unwrap();
  let unreadable = tick(
    &request(&state, NOW, 165),
    Source::File(&recorded_165()),
    &Env::default(),
  );
  assert_eq!(unreadable.err().unwrap().code, "state_malformed");
  let state = dir.path().join("t.json");
  tick(
    &request(&state, NOW, 165),
    Source::File(&recorded_165()),
    &Env::default(),
  )
  .unwrap();
  let missing = dir.path().join("missing.json");
  let failed = tick(
    &request(&state, NOW, 165),
    Source::File(&missing),
    &Env::default(),
  );
  assert_eq!(failed.err().unwrap().code, "invalid_json");
  assert_eq!(
    read(&state)["pr"]["lastError"],
    Value::Null,
    "TypeScript stamps only collection"
  );
}
