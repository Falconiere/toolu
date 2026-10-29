use super::{execute, snapshot};
use crate::herdr::Call;
use crate::paths::Paths;
use serde_json::json;
use toolu_mux::Backend;
use toolu_runtime::env::Env;

#[test]
fn snapshot_uses_reported_status_and_exact_remote_identity() {
  let value = snapshot(&json!({"terminals":[
    {"address":"vps/t7","cwd":"/work tree","state":"live","labels":{"toolu.owner":"run"},"status":{"state":"blocked"}},
    {"address":"t8","labels":{}}
  ]}));
  assert_eq!(value["agents"].as_array().unwrap().len(), 1);
  assert_eq!(value["agents"][0]["name"], "hc:vps/t7");
  assert_eq!(value["agents"][0]["agent_status"], "blocked");
}

#[test]
fn legacy_targets_are_never_sent_to_hc() {
  let env = Env::from_pairs([
    ("PATH", "/nonexistent"),
    ("TOOLU_EPIC_MULTIPLEXER", "herdr-app"),
  ]);
  assert!(
    execute(
      &Paths::at("/tmp"),
      &env,
      Backend::HerdrApp,
      &Call::Read { target: "a".into() }
    )
    .is_none()
  );
  let output = execute(
    &Paths::at("/tmp"),
    &env,
    Backend::HerdrApp,
    &Call::Read {
      target: "hc:t7".into(),
    },
  )
  .unwrap()
  .unwrap();
  assert!(output.contains("identity_mismatch"));
}
