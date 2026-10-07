use serde_json::{Map, Value, json};
use toolu_protocol::host::Host;

use super::{EpicError, EpicSettings, Peer, parse};
use crate::config::load::LoadedConfig;

fn config(epic: Value) -> LoadedConfig {
  let mut data = Map::new();
  data.insert("epic".to_owned(), epic);
  LoadedConfig::from_data(data, Host::Codex)
}

#[test]
fn defaults_and_safe_unknown_keys_resolve() {
  let empty = LoadedConfig::from_data(Map::new(), Host::Codex);
  let defaults = parse(&empty).unwrap();
  assert_eq!(defaults.http_bind, "127.0.0.1");
  assert_eq!(defaults.http_port, 7717);
  assert!(!defaults.attention_enabled);
  assert_eq!(defaults.peers, []);

  let settings = parse(&config(json!({
    "http": {"bind": "0.0.0.0", "port": 8123, "future": true},
    "attention": {"enabled": true},
    "peers": [{"name": "remote", "url": "https://peer.example:8443"}],
    "future": {"switch": 2}
  })))
  .unwrap();
  assert_eq!(settings.http_bind, "0.0.0.0");
  assert_eq!(settings.http_port, 8123);
  assert!(settings.attention_enabled);
  assert_eq!(settings.peers.len(), 1);
  let _: &Peer = &settings.peers[0];
  assert_eq!(settings.peers[0].name, "remote");
  assert_eq!(settings.peers[0].url, "https://peer.example:8443/");
}

#[test]
fn credentials_and_bad_settings_are_rejected_without_values() {
  let cases = [
    (json!({"status_token": "sentinel-one"}), "epic.status_token"),
    (
      json!({"attention": {"notifyUrl": "sentinel-two"}}),
      "epic.attention.notifyUrl",
    ),
    (json!({"http": {"port": 0}}), "epic.http.port"),
    (json!({"http": {"bind": 1}}), "epic.http.bind"),
    (
      json!({"peers": [{"name":"a","url":"https://host/a-secret"}]}),
      "epic.peers[0].url",
    ),
    (
      json!({"peers": [{"name":"a","url":"https://host/?token=x"}]}),
      "epic.peers[0].url",
    ),
    (
      json!({"peers": [{"name":"a","url":"https://user:pass@host"}]}),
      "epic.peers[0].url",
    ),
    (
      json!({"peers": [{"name":"a","url":"https://host"},{"name":"a","url":"https://other"}]}),
      "epic.peers[1].name",
    ),
  ];
  for (value, path) in cases {
    let rejected: EpicError = parse(&config(value)).unwrap_err();
    let error = rejected.to_string();
    assert!(error.contains(path), "{error}");
    for secret in ["sentinel-one", "sentinel-two", "a-secret", "user:pass"] {
      assert!(!error.contains(secret), "{error}");
    }
  }
}

#[test]
fn malformed_epic_section_and_invalid_envelope_fail_closed() {
  assert!(parse(&config(json!(false))).is_err());
  let mut invalid = LoadedConfig::from_data(Map::new(), Host::Codex);
  invalid.invalid = Some("unknown top-level key 'unsafe'".to_owned());
  assert!(parse(&invalid).is_err());
}

#[test]
fn public_settings_type_is_stable_for_consumers() {
  let _: EpicSettings = parse(&config(json!({}))).unwrap();
}
