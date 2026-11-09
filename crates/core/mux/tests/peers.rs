//! Black-box relay peer protocol tests.
#![cfg(test)]
use serde_json::{Value, json};
use std::path::Path;
use toolu_mux::relay::{
  config::{self, Config, Machine},
  rpc,
  store::Store,
};
use toolu_runtime::env::Env;

const SECRET: &str = "0123456789abcdef0123456789abcdef";
const KEY: &str = "abcdef0123456789abcdef0123456789";

fn invoke(root: &Path, value: &Value) -> Value {
  rpc::receive(
    root,
    &Env::from_pairs([("PATH", "/usr/bin")]),
    &value.to_string(),
  )
  .unwrap()
}
fn setup(root: &Path) {
  let mut cfg = Config {
    relay: Some("relay".into()),
    generation: Some("foundation".into()),
    ..Config::default()
  };
  for name in ["one", "two"] {
    config::register(
      &mut cfg,
      Machine {
        name: name.into(),
        root: format!("/tmp/{name}"),
        agents: vec!["codex".into(), "claude".into()],
        capacity: 1,
        draining: false,
      },
    )
    .unwrap();
  }
  invoke(
    root,
    &json!({"op":"enroll", "expected":Config::default(), "config":cfg}),
  );
}
fn request(action: &str) -> Value {
  json!({"op":"peers", "version":1, "authority":"foundation", "action":action})
}
fn auth(id: &str) -> Value {
  json!({"id":id, "generation":"gen", "secret":SECRET})
}
fn session(id: &str, group: &str) -> Value {
  json!({"id":id, "generation":"gen", "machine":"one", "host":"codex", "group":group, "active":true, "wake":null})
}
fn join(root: &Path, id: &str, group: &str) -> Value {
  let mut req = request("join");
  req["session"] = session(id, group);
  req["secret"] = json!(SECRET);
  req["group_key"] = json!(KEY);
  invoke(root, &req)
}
fn call(root: &Path, action: &str, id: &str) -> Value {
  let mut req = request(action);
  req["auth"] = auth(id);
  invoke(root, &req)
}
fn error(value: &Value, code: &str) {
  assert_eq!(value["error"]["code"], code, "{value}");
}
fn envelope(sender: &str, recipient: &str, group: &str, id: &str, text: &str) -> Value {
  json!({"version":1, "sender":{"id":sender,"generation":"gen"}, "recipient":{"id":recipient,"generation":"gen"}, "group":group, "conversation_id":id, "message_id":id, "reply_to":null, "timestamp":1, "text":text})
}
fn send(root: &Path, sender: &str, envelope: Value) -> Value {
  let mut req = request("send");
  req["auth"] = auth(sender);
  req["envelope"] = envelope;
  invoke(root, &req)
}
fn keyed(root: &Path, action: &str, id: &str, key: &str) -> Value {
  let mut req = request(action);
  req["auth"] = auth(id);
  req["key"] = json!(key);
  invoke(root, &req)
}

#[test]
fn membership_is_private_idempotent_and_fenced() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  setup(root);
  let first = join(root, "alice", "team");
  assert_eq!(first["version"], 1);
  assert_eq!(join(root, "alice", "team"), first);
  join(root, "bob", "team");
  join(root, "outsider", "other");
  let list = call(root, "list", "alice");
  assert_eq!(list["result"].as_array().unwrap().len(), 2);
  let text = list.to_string();
  assert!(!text.contains(SECRET));
  assert!(!text.contains(KEY));
  assert!(!text.contains("outsider"));
  let mut bad = request("join");
  bad["session"] = session("intruder", "team");
  bad["secret"] = json!(SECRET);
  bad["group_key"] = json!("x".repeat(32));
  error(&invoke(root, &bad), "unauthorized");
  call(root, "leave", "bob");
  call(root, "leave", "bob");
  error(&join(root, "bob", "team"), "stale");
  assert_eq!(
    call(root, "list", "alice")["result"]
      .as_array()
      .unwrap()
      .len(),
    1
  );
}

#[test]
fn delivery_retries_ack_and_replies_survive_reopening() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  setup(root);
  let before_config = config::load(root).unwrap();
  let before_tasks = Store::new(root).list().unwrap();
  for id in ["alice", "bob", "eve"] {
    join(root, id, "team");
  }
  let original = envelope("alice", "bob", "team", "m1", "hello");
  let sent = send(root, "alice", original.clone());
  assert_eq!(
    sent["result"],
    json!({"key":"alice/m1", "envelope":original, "acknowledged":false, "wake":"unsupported"})
  );
  assert_eq!(send(root, "alice", original.clone()), sent);
  let mut changed = original;
  changed["text"] = json!("changed");
  error(&send(root, "alice", changed), "conflict");
  let inbox = call(root, "inbox", "bob");
  assert_eq!(inbox["result"]["messages"].as_array().unwrap().len(), 1);
  assert_eq!(inbox["result"]["unread"], 1);
  assert_eq!(call(root, "inbox", "bob")["result"]["unread"], 1);
  error(&keyed(root, "ack", "alice", "alice/m1"), "unauthorized");
  error(&keyed(root, "status", "eve", "alice/m1"), "unauthorized");
  assert_eq!(
    keyed(root, "ack", "bob", "alice/m1")["result"]["acknowledged"],
    true
  );
  assert_eq!(
    keyed(root, "status", "alice", "alice/m1")["result"]["acknowledged"],
    true
  );
  let mut reply = envelope("bob", "alice", "team", "m2", "reply");
  reply["conversation_id"] = json!("m1");
  reply["reply_to"] = json!("alice/m1");
  assert_eq!(send(root, "bob", reply.clone())["result"]["key"], "bob/m2");
  reply["conversation_id"] = json!("wrong");
  reply["message_id"] = json!("m3");
  error(&send(root, "bob", reply), "invalid");
  assert_eq!(config::load(root).unwrap(), before_config);
  assert_eq!(Store::new(root).list().unwrap(), before_tasks);
}

#[test]
fn validates_protocol_identity_and_utf8_byte_limit() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  setup(root);
  join(root, "alice", "team");
  join(root, "bob", "team");
  join(root, "eve", "other");
  rejects_invalid_protocol_auth(root);
  let mut bad = request("join");
  bad["session"] = session("bad/id", "team");
  bad["secret"] = json!(SECRET);
  bad["group_key"] = json!(KEY);
  error(&invoke(root, &bad), "invalid");
  bad["session"] = session("new", "team");
  bad["session"]["machine"] = json!("unknown");
  error(&invoke(root, &bad), "unauthorized");
  error(
    &send(
      root,
      "alice",
      envelope("alice", "eve", "team", "cross", "x"),
    ),
    "unauthorized",
  );
  assert_eq!(
    send(
      root,
      "alice",
      envelope("alice", "bob", "team", "limit", &"é".repeat(32768))
    )["result"]["key"],
    "alice/limit"
  );
  error(
    &send(
      root,
      "alice",
      envelope("alice", "bob", "team", "over", &"é".repeat(32769)),
    ),
    "invalid",
  );
}

fn rejects_invalid_protocol_auth(root: &Path) {
  let mut req = request("list");
  req["auth"] = auth("alice");
  req["version"] = json!(2);
  error(&invoke(root, &req), "invalid");
  req["version"] = json!(1);
  req["authority"] = json!("old");
  error(&invoke(root, &req), "stale");
  req["authority"] = json!("foundation");
  req["auth"]["secret"] = json!("x".repeat(32));
  error(&invoke(root, &req), "unauthorized");
  req["auth"] = auth("alice");
  req["auth"]["generation"] = json!("old");
  error(&invoke(root, &req), "stale");
}

#[test]
fn concurrent_sends_are_durable_and_paginated_without_loss() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  setup(root);
  join(root, "alice", "team");
  join(root, "bob", "team");
  std::thread::scope(|scope| {
    for n in 0..40 {
      scope.spawn(move || {
        let id = format!("msg{n}");
        let result = send(root, "alice", envelope("alice", "bob", "team", &id, "x"));
        assert_eq!(result["result"]["key"], format!("alice/{id}"));
      });
    }
  });
  let first = call(root, "inbox", "bob");
  assert_eq!(first["result"]["messages"].as_array().unwrap().len(), 32);
  assert_eq!(first["result"]["unread"], 40);
  let mut req = request("inbox");
  req["auth"] = auth("bob");
  req["after"] = first["result"]["next"].clone();
  let second = invoke(root, &req);
  assert_eq!(second["result"]["messages"].as_array().unwrap().len(), 8);
  assert_eq!(second["result"]["next"], Value::Null);
  let keys: std::collections::HashSet<_> = first["result"]["messages"]
    .as_array()
    .unwrap()
    .iter()
    .chain(second["result"]["messages"].as_array().unwrap())
    .map(|message| message["key"].as_str().unwrap())
    .collect();
  assert_eq!(keys.len(), 40);
}

#[test]
fn a_send_waits_for_a_relay_lock_held_past_five_seconds() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  setup(root);
  join(root, "alice", "team");
  join(root, "bob", "team");
  let (held, ready) = std::sync::mpsc::channel();
  std::thread::scope(|scope| {
    scope.spawn(move || {
      let options = toolu_state::lock::LockOptions {
        stale: std::time::Duration::MAX,
        ..Default::default()
      };
      toolu_state::lock::with_lock_strict(&root.join("rpc"), options, || {
        held.send(()).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5_500));
      })
      .unwrap();
    });
    ready.recv().unwrap();
    let result = send(root, "alice", envelope("alice", "bob", "team", "late", "x"));
    assert_eq!(result["result"]["key"], "alice/late");
  });
}
