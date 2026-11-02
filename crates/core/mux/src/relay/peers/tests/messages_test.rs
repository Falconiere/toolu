use super::super::tests::{auth, code, envelope, join, request, send, setup};
use crate::relay::peers::receive;
use serde_json::json;

#[test]
fn escaped_text_pages_fit_the_existing_relay_transport_limit() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  setup(root);
  join(root, "alice");
  join(root, "bob");
  for index in 0..12 {
    let mut message = envelope("alice", "bob", &format!("m{index}"));
    message["text"] = json!("\0".repeat(65536));
    assert!(send(root, "alice", message).get("result").is_some());
  }
  let mut req = request("inbox");
  req["auth"] = auth("bob");
  let first = receive(root, &req);
  assert!(serde_json::to_vec(&first).unwrap().len() < 8 * 1024 * 1024);
  assert!(first["result"]["next"].is_string());
  req["after"] = first["result"]["next"].clone();
  let second = receive(root, &req);
  let mut keys = std::collections::BTreeSet::new();
  for page in [&first, &second] {
    assert!(serde_json::to_vec(page).unwrap().len() < 8 * 1024 * 1024);
    for message in page["result"]["messages"].as_array().unwrap() {
      assert!(keys.insert(message["key"].as_str().unwrap()));
    }
  }
  assert_eq!(keys.len(), 12);
  assert_eq!(second["result"]["next"], serde_json::Value::Null);
  assert_eq!(second["result"]["unread"], 12);
}

#[test]
fn stale_recipient_and_outsider_reply_are_rejected() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  setup(root);
  for id in ["alice", "bob", "eve"] {
    join(root, id);
  }
  let mut stale = envelope("alice", "bob", "stale");
  stale["recipient"]["generation"] = json!("old");
  code(&send(root, "alice", stale), "stale");
  assert_eq!(
    send(root, "alice", envelope("alice", "bob", "first"))["result"]["key"],
    "alice/first"
  );
  let mut reply = envelope("eve", "alice", "second");
  reply["conversation_id"] = json!("first");
  reply["reply_to"] = json!("alice/first");
  code(&send(root, "eve", reply), "invalid");
}
