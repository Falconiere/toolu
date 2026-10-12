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

#[test]
fn a_status_always_carries_pr_null_until_a_report_names_one() {
  let dir = tempfile::tempdir().unwrap();
  let path = dir.path().join("status.json");
  write_status(&path, "execution", None, "", std::time::UNIX_EPOCH).unwrap();
  assert_eq!(read_value(&path).unwrap()["pr"], serde_json::Value::Null);
  write_status(&path, "pr-open", Some(7), "", std::time::UNIX_EPOCH).unwrap();
  write_status(&path, "babysit", None, "", std::time::UNIX_EPOCH).unwrap();
  assert_eq!(read_value(&path).unwrap()["pr"], 7);
}
