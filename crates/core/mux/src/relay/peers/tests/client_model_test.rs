use super::{outbox, state};
use serde_json::json;

#[test]
fn local_state_rejects_wrong_version_or_identity_without_rewriting() {
  let dir = tempfile::tempdir().unwrap();
  let path = dir.path().join("alice/state.json");
  std::fs::create_dir_all(path.parent().unwrap()).unwrap();
  let session = json!({"id":"alice","generation":"gen","machine":"one","host":"codex","group":"team","active":true,"wake":null});
  let mut value = json!({"version":1,"session":session,"secret":"secret","relay":"relay","authority":"foundation","group_key":"key"});
  std::fs::write(&path, value.to_string()).unwrap();
  assert!(state(&path).is_ok());
  value["version"] = json!(2);
  let bytes = value.to_string();
  std::fs::write(&path, &bytes).unwrap();
  assert!(state(&path).is_err());
  assert_eq!(std::fs::read_to_string(&path).unwrap(), bytes);
  value["version"] = json!(1);
  value["session"]["id"] = json!("other");
  std::fs::write(&path, value.to_string()).unwrap();
  assert!(state(&path).is_err());
}

#[test]
fn outbox_rejects_future_versions_and_malformed_entries() {
  let dir = tempfile::tempdir().unwrap();
  let path = dir.path().join("outbox.json");
  for value in [
    json!({"version":2,"messages":{}}),
    json!({"version":1,"messages":{"m1":{"delivery":"queued"}}}),
  ] {
    let bytes = value.to_string();
    std::fs::write(&path, &bytes).unwrap();
    assert!(outbox(&path).is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), bytes);
  }
}
