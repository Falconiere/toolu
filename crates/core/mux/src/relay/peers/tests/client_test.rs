use super::{Join, Send, prepare, register, result, send};
use crate::relay::config;
use serde_json::{Value, json};
use std::{path::PathBuf, time::Duration};
use toolu_runtime::env::Env;

#[test]
fn malformed_and_error_responses_never_succeed() {
  for response in [
    json!(null),
    json!({"version":2,"result":{}}),
    json!({"version":1}),
    json!({"version":1,"error":{"code":"unauthorized","message":"denied","retryable":false}}),
  ] {
    assert!(result(response).is_err());
  }
  assert_eq!(
    result(json!({"version":1,"result":{"unread":2}})).unwrap(),
    json!({"unread":2})
  );
}

struct Fixture {
  dir: tempfile::TempDir,
  env: Env,
  key: PathBuf,
}
impl Fixture {
  fn new() -> Self {
    let dir = tempfile::tempdir().unwrap();
    let env = Env::from_pairs([
      ("TOOLU_CONFIG_DIR", dir.path().to_str().unwrap()),
      ("PATH", "/definitely/absent"),
    ]);
    config::update(&config::root(&env), |cfg| {
      cfg.relay = Some("relay".into());
      cfg.generation = Some("foundation".into());
      Ok(())
    })
    .unwrap();
    let key = dir.path().join("group-key");
    std::fs::write(&key, "0123456789abcdef0123456789abcdef").unwrap();
    Self { dir, env, key }
  }
  fn prepare(&self, group: &str) -> Result<String, String> {
    prepare(
      &self.env,
      &Join {
        group,
        machine: "one",
        host: "codex",
        key_file: self.key.to_str().unwrap(),
      },
    )
  }
  fn path(&self, id: &str, file: &str) -> PathBuf {
    self
      .dir
      .path()
      .join("delegation/peers/clients")
      .join(id)
      .join(file)
  }
  fn outbox(&self, id: &str) -> Value {
    serde_json::from_slice(&std::fs::read(self.path(id, "outbox.json")).unwrap()).unwrap()
  }
  fn send(&self, id: &str, text: &str) -> Result<Value, String> {
    send(
      &self.env,
      id,
      &Send {
        recipient: json!({"id":"bob","generation":"gen"}),
        text,
        message_id: Some("m1"),
        reply_to: None,
        conversation_id: None,
      },
    )
  }
}

#[test]
fn offline_registration_and_message_retries_preserve_durable_receipts() {
  let fixture = Fixture::new();
  let id = fixture.prepare("team").unwrap();
  assert!(!id.is_empty() && id.len() <= 128);
  assert!(
    id.bytes()
      .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
  );
  assert!(register(&fixture.env, &id, Duration::from_secs(1)).is_err());
  assert!(fixture.path(&id, "state.json").is_file());
  let first = fixture.send(&id, "hello").unwrap();
  assert_eq!(first["delivery"], "queued");
  assert_eq!(
    first["receipt"],
    json!({"outbox":true,"inbox":false,"acknowledged":false})
  );
  let saved = fixture.outbox(&id);
  assert_eq!(saved["messages"]["m1"]["envelope"], first["envelope"]);
  let retry = fixture.send(&id, "hello").unwrap();
  assert_eq!(retry["envelope"], first["envelope"]);
  assert_eq!(
    fixture.outbox(&id)["messages"].as_object().unwrap().len(),
    1
  );
  assert!(fixture.send(&id, "changed").is_err());
  assert_eq!(
    fixture.outbox(&id)["messages"]["m1"]["envelope"],
    first["envelope"]
  );
}

#[test]
fn corrupt_and_future_local_state_is_never_rewritten() {
  for file in ["state.json", "outbox.json"] {
    for content in ["{broken", r#"{"version":2,"messages":{}}"#] {
      let fixture = Fixture::new();
      let id = fixture.prepare("team").unwrap();
      let path = fixture.path(&id, file);
      std::fs::write(&path, content).unwrap();
      assert!(fixture.send(&id, "hello").is_err());
      assert_eq!(std::fs::read_to_string(path).unwrap(), content);
    }
  }
}

#[test]
fn unsafe_identifiers_and_oversized_text_are_rejected_before_queueing() {
  let fixture = Fixture::new();
  assert!(fixture.prepare("../team").is_err());
  let id = fixture.prepare("team").unwrap();
  assert!(fixture.send("../escape", "hello").is_err());
  assert!(fixture.send(&id, &"é".repeat(32769)).is_err());
  let invalid = Send {
    recipient: json!({"id":"../bob","generation":"gen"}),
    text: "hello",
    message_id: Some("m1"),
    reply_to: None,
    conversation_id: None,
  };
  assert!(send(&fixture.env, &id, &invalid).is_err());
  assert_eq!(fixture.outbox(&id)["messages"], json!({}));
}
