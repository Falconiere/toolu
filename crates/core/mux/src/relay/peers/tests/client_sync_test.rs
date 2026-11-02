use super::{local, sync};
use serde_json::json;
use std::time::Duration;
use toolu_runtime::env::Env;

#[test]
fn offline_sync_returns_queued_original_and_receipts() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path().to_str().unwrap();
  let env = Env::from_pairs([("TOOLU_CONFIG_DIR", root), ("PATH", "/definitely/absent")]);
  let path = dir.path().join("delegation/peers/clients/alice");
  std::fs::create_dir_all(&path).unwrap();
  let session = json!({"id":"alice","generation":"gen","machine":"one","host":"codex","group":"team","active":true,"wake":null});
  std::fs::write(path.join("state.json"), json!({"version":1,"session":session,"secret":"secret","relay":"relay","authority":"foundation","group_key":"key"}).to_string()).unwrap();
  let envelope = json!({"version":1,"sender":{"id":"alice","generation":"gen"},"recipient":{"id":"bob","generation":"gen"},"group":"team","conversation_id":"m1","message_id":"m1","reply_to":null,"timestamp":123,"text":"hello"});
  let item = json!({"envelope":envelope,"delivery":"queued","receipt":{"outbox":true,"inbox":false,"acknowledged":false}});
  std::fs::write(
    path.join("outbox.json"),
    json!({"version":1,"messages":{"m1":item}}).to_string(),
  )
  .unwrap();
  let result = sync(&env, "alice", Some("alice/m1"), 1, Duration::from_secs(1)).unwrap();
  assert_eq!(result["messages"]["m1"]["envelope"], envelope);
  assert_eq!(result["messages"]["m1"]["delivery"], "queued");
  assert_eq!(result["messages"]["m1"]["receipt"], item["receipt"]);
  assert!(result["messages"]["m1"]["error"].is_string());
  let message =
    json!({"key":"alice/m1","envelope":envelope,"acknowledged":false,"wake":"unsupported"});
  let delivered = json!({"envelope":envelope,"delivery":"delivered","receipt":{"outbox":true,"inbox":true,"acknowledged":false},"message":message});
  std::fs::write(
    path.join("outbox.json"),
    json!({"version":1,"messages":{"m1":delivered}}).to_string(),
  )
  .unwrap();
  for result in [
    local(&env, "alice", Some("alice/m1"), 1).unwrap(),
    sync(&env, "alice", Some("alice/m1"), 1, Duration::from_secs(1)).unwrap(),
  ] {
    assert_eq!(result["messages"]["m1"], delivered);
  }
}
