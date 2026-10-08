use serde_json::json;

use super::{read_value, write_status, write_value};

#[test]
fn a_status_rewrite_keeps_an_extra_key_and_appends_history() {
  let dir = tempfile::tempdir().unwrap();
  let path = dir.path().join("status.json");
  write_value(&path, &json!({"agent": "worker", "phase": "plan"})).unwrap();
  write_status(&path, "ready", Some(12), "note", std::time::UNIX_EPOCH).unwrap();
  let value = read_value(&path).unwrap();
  assert_eq!(value["agent"], "worker");
  assert_eq!(value["phase"], "ready");
  assert_eq!(value["pr"], 12);
  assert_eq!(value["history"][0]["phase"], "ready");
}
