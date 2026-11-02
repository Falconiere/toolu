use super::notice;
use toolu_mux::relay::config;
use toolu_protocol::host::Host;
use toolu_runtime::env::Env;

const SECRET: &str = "0123456789abcdef0123456789abcdef";

fn fixture() -> (tempfile::TempDir, Env) {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path().to_str().unwrap();
  let base = Env::from_pairs([("TOOLU_CONFIG_DIR", root), ("PATH", "/definitely/absent")]);
  assert_eq!(
    notice(&base, Host::Codex, r#"{"session_id":"host"}"#, "startup"),
    None
  );
  config::update(&config::root(&base), |cfg| {
    cfg.relay = Some("relay".into());
    cfg.generation = Some("foundation".into());
    Ok(())
  })
  .unwrap();
  let key = dir.path().join("key");
  std::fs::write(&key, SECRET).unwrap();
  let env = base
    .with("TOOLU_PEERS_GROUP", "team")
    .with("TOOLU_PEERS_MACHINE", "one")
    .with("TOOLU_PEERS_KEY_FILE", key.to_str().unwrap());
  (dir, env)
}

#[test]
fn opt_in_binding_survives_offline_resume_and_changes_on_startup() {
  let (_dir, env) = fixture();
  let payload = r#"{"session_id":"host","text":"private message"}"#;
  let first = notice(&env, Host::Codex, payload, "startup").unwrap();
  let binding = config::root(&env).join("peers/bindings/codex-host");
  let id = std::fs::read_to_string(&binding).unwrap();
  let state_path = config::root(&env)
    .join("peers/clients")
    .join(&id)
    .join("state.json");
  let state: serde_json::Value =
    serde_json::from_slice(&std::fs::read(state_path).unwrap()).unwrap();
  assert!(first.contains(&id));
  assert!(!first.contains(SECRET));
  assert!(!first.contains("private message"));
  assert!(!first.contains(state["secret"].as_str().unwrap()));
  assert!(first.len() < 512);
  for event in ["resume", "prompt"] {
    assert!(
      notice(&env, Host::Codex, payload, event)
        .unwrap()
        .contains(&id)
    );
    assert_eq!(std::fs::read_to_string(&binding).unwrap(), id);
  }
  notice(&env, Host::Codex, payload, "startup").unwrap();
  let next = std::fs::read_to_string(binding).unwrap();
  assert_ne!(next, id);
  let next_path = config::root(&env)
    .join("peers/clients")
    .join(next)
    .join("state.json");
  let next_state: serde_json::Value =
    serde_json::from_slice(&std::fs::read(next_path).unwrap()).unwrap();
  assert_ne!(
    state["session"]["generation"],
    next_state["session"]["generation"]
  );
}
