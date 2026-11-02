//! Real CLI conversations over isolated HC cores.
#![cfg(test)]
#[cfg(test)]
#[path = "helpers/peers_fixture.rs"]
mod fixture;
use fixture::Fixture;
use serde_json::{Value, json};

fn identity(value: &Value) -> (&str, &str) {
  (
    value["session_id"].as_str().unwrap(),
    value["session"]["generation"].as_str().unwrap(),
  )
}

#[test]
fn conversation_and_lost_receipt_retry_use_the_real_transport() {
  let world = Fixture::new();
  let relay_config = std::fs::read(world.root("relay").join("delegation/config.json")).unwrap();
  let tasks = world.rpc(&json!({"op":"list"}));
  let alice = world.join("alice", "key");
  let bob = world.join("bob", "key");
  let (a, _) = identity(&alice);
  let (b, bg) = identity(&bob);
  let sent = world.peer(
    "alice",
    a,
    &[
      "send",
      "--to",
      b,
      "--generation",
      bg,
      "--text",
      "question",
      "--id",
      "m1",
    ],
  );
  assert_eq!(sent["delivery"], "delivered");
  assert_eq!(
    sent["receipt"],
    json!({"outbox":true,"inbox":true,"acknowledged":false})
  );
  let key = format!("{a}/m1");
  let inbox = world.peer("bob", b, &["inbox"]);
  assert_eq!(inbox["messages"].as_array().unwrap().len(), 1);
  assert_eq!(inbox["unread"], 1);
  assert_eq!(world.peer("bob", b, &["inbox"])["unread"], 1);
  verify_retry_and_ack(&world, a, b, &key, &sent);
  let reply = world.peer("bob", b, &["reply", &key, "--text", "answer"]);
  assert_eq!(reply["envelope"]["reply_to"], key);
  assert_eq!(reply["envelope"]["conversation_id"], "m1");
  assert_eq!(world.peer("alice", a, &["inbox"])["unread"], 1);
  assert_eq!(world.rpc(&json!({"op":"list"})), tasks);
  assert_eq!(
    std::fs::read(world.root("relay").join("delegation/config.json")).unwrap(),
    relay_config
  );
}

fn verify_retry_and_ack(world: &Fixture, a: &str, b: &str, key: &str, sent: &Value) {
  let path = world
    .root("alice")
    .join("delegation/peers/clients")
    .join(a)
    .join("outbox.json");
  let mut outbox: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
  outbox["messages"]["m1"]["delivery"] = json!("queued");
  std::fs::write(path, outbox.to_string()).unwrap();
  let retried = world.peer("alice", a, &["status", key]);
  assert_eq!(retried["messages"]["m1"]["envelope"], sent["envelope"]);
  assert_eq!(
    world.peer("bob", b, &["inbox"])["messages"]
      .as_array()
      .unwrap()
      .len(),
    1
  );
  assert_eq!(world.peer("bob", b, &["ack", key])["acknowledged"], true);
  let refreshed = world.peer("alice", a, &["status", key]);
  assert_eq!(refreshed["messages"]["m1"]["receipt"]["acknowledged"], true);
  assert_eq!(refreshed["messages"]["m1"]["message"]["acknowledged"], true);
}

#[test]
fn credentials_stale_recipients_and_protocol_version_are_rejected() {
  let world = Fixture::new();
  let alice = world.join("alice", "key");
  let bob = world.join("bob", "key");
  let (a, _) = identity(&alice);
  let (b, _) = identity(&bob);
  let bad = world.cli_output(
    "eve",
    &[
      "peers",
      "join",
      "--group",
      "team",
      "--machine",
      "one",
      "--agent",
      "codex",
      "--key-file",
      &world.key("bad-key"),
    ],
    false,
  );
  assert!(!bad.status.success());
  let stale = world.peer(
    "alice",
    a,
    &[
      "send",
      "--to",
      b,
      "--generation",
      "old",
      "--text",
      "x",
      "--id",
      "stale",
    ],
  );
  assert_eq!(stale["delivery"], "rejected");
  assert_eq!(stale["error"]["code"], "stale");
  verify_offline_retry(&world, a, b, identity(&bob).1);
  let response =
    world.rpc(&json!({"op":"peers","version":2,"authority":"foundation","action":"list"}));
  assert_eq!(response["version"], 1);
  assert_eq!(response["error"]["code"], "invalid");
}

fn verify_offline_retry(world: &Fixture, a: &str, b: &str, bg: &str) {
  let offline = world.cli_output(
    "alice",
    &[
      "peers",
      "--session",
      a,
      "send",
      "--to",
      b,
      "--generation",
      bg,
      "--text",
      "offline",
      "--id",
      "offline",
    ],
    true,
  );
  assert!(offline.status.success());
  let queued: Value = serde_json::from_slice(&offline.stdout).unwrap();
  assert_eq!(queued["delivery"], "queued");
  let offline_status = world.cli_output(
    "alice",
    &["peers", "--session", a, "status", "offline"],
    true,
  );
  assert!(offline_status.status.success());
  let saved: Value = serde_json::from_slice(&offline_status.stdout).unwrap();
  assert!(saved["registration_error"].is_string());
  assert_eq!(saved["messages"]["offline"]["envelope"], queued["envelope"]);
  assert_eq!(saved["messages"]["offline"]["receipt"], queued["receipt"]);
  let retried = world.peer("alice", a, &["status", "offline"]);
  assert_eq!(retried["messages"]["offline"]["delivery"], "delivered");
  assert_eq!(
    retried["messages"]["offline"]["envelope"],
    queued["envelope"]
  );
}
