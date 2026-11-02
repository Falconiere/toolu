use super::receive;
use crate::relay::{
  config::{Config, Machine},
  rpc,
};
use serde_json::{Value, json};
use std::path::Path;
use toolu_runtime::env::Env;

pub(super) const SECRET: &str = "0123456789abcdef0123456789abcdef";
pub(super) fn setup(root: &Path) {
  let mut cfg = Config {
    relay: Some("relay".into()),
    generation: Some("foundation".into()),
    ..Config::default()
  };
  cfg.machines.insert(
    "one".into(),
    Machine {
      name: "one".into(),
      root: "/tmp/one".into(),
      agents: vec!["codex".into()],
      capacity: 1,
      draining: false,
    },
  );
  rpc::receive(
    root,
    &Env::from_pairs([("PATH", "/usr/bin")]),
    &json!({"op":"enroll", "expected":Config::default(), "config":cfg}).to_string(),
  )
  .unwrap();
}
pub(super) fn request(action: &str) -> Value {
  json!({"op":"peers","version":1,"authority":"foundation","action":action})
}
pub(super) fn session(id: &str) -> Value {
  json!({"id":id,"generation":"gen","machine":"one","host":"codex","group":"team","active":true,"wake":null})
}
pub(super) fn auth(id: &str) -> Value {
  json!({"id":id,"generation":"gen","secret":SECRET})
}
pub(super) fn join(root: &Path, id: &str) -> Value {
  let mut req = request("join");
  req["session"] = session(id);
  req["secret"] = json!(SECRET);
  req["group_key"] = json!(SECRET);
  super::receive(root, &req)
}
pub(super) fn code(value: &Value, expected: &str) {
  assert_eq!(value["error"]["code"], expected, "{value}");
}
pub(super) fn envelope(sender: &str, recipient: &str, id: &str) -> Value {
  json!({"version":1,"sender":{"id":sender,"generation":"gen"},"recipient":{"id":recipient,"generation":"gen"},"group":"team","conversation_id":id,"message_id":id,"reply_to":null,"timestamp":1,"text":"hello"})
}
pub(super) fn send(root: &Path, sender: &str, envelope: Value) -> Value {
  let mut req = request("send");
  req["auth"] = auth(sender);
  req["envelope"] = envelope;
  super::receive(root, &req)
}

#[test]
fn malformed_nested_objects_return_structured_invalid_errors() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  setup(root);
  join(root, "alice");
  join(root, "bob");
  let mut unknown = request("list");
  unknown["auth"] = auth("alice");
  unknown["extra"] = json!(true);
  code(&receive(root, &unknown), "invalid");
  let mut req = request("list");
  req["auth"] = auth("alice");
  req["auth"]["extra"] = json!(true);
  code(&receive(root, &req), "invalid");
  let mut message = envelope("alice", "bob", "first");
  message["extra"] = json!(true);
  code(&send(root, "alice", message), "invalid");
  let mut message = envelope("alice", "bob", "second");
  message["sender"]["extra"] = json!(true);
  code(&send(root, "alice", message), "invalid");
}
