use std::time::Duration;

use toolu_runtime::env::Env;

use crate::{Config, DEFAULT_MODEL, ENDPOINT, Error, Jev, timeout};

#[test]
fn the_key_is_required_and_refused_with_a_line_break() {
  assert_eq!(
    Jev::from_env(&Env::default(), Config::default()).map(|_| ()),
    Err(Error::MissingKey)
  );
  for key in ["bad\nkey", "bad\rkey"] {
    let env = Env::from_pairs([("TYPESAFE_API_KEY", key)]);
    assert_eq!(
      Jev::from_env(&env, Config::default()).map(|_| ()),
      Err(Error::KeyLineBreak)
    );
  }
}

#[test]
fn jev_timeout_is_seconds_with_a_60_second_default() {
  assert_eq!(timeout(None), Duration::from_secs(60));
  assert_eq!(timeout(Some("1.5")), Duration::from_millis(1500));
  assert_eq!(timeout(Some(" 5 ")), Duration::from_secs(5));
  assert_eq!(timeout(Some("0")), Duration::ZERO);
  for bad in ["-1", "soon", "inf", "1e400", "1e19"] {
    assert_eq!(timeout(Some(bad)), Duration::from_secs(60), "{bad}");
  }
}

#[test]
fn debug_shows_the_endpoint_and_timeout_but_never_the_key() {
  let env = Env::from_pairs([("TYPESAFE_API_KEY", "secret-key-460"), ("JEV_TIMEOUT", "0")]);
  let jev = Jev::from_env(&env, Config::default()).expect("jev");
  let shown = format!("{jev:?}");
  assert!(
    shown.starts_with("Jev { endpoint: \"https://api.typesafe.ai/v1/systemone\""),
    "{shown}"
  );
  assert!(!shown.contains("secret-key-460"), "{shown}");
}

#[test]
fn the_default_config_targets_jev_with_one_second_pauses() {
  let config = Config::default();
  assert_eq!(config.endpoint, ENDPOINT);
  assert_eq!(config.endpoint, "https://api.typesafe.ai/v1/systemone");
  assert_eq!(config.pause, Duration::from_secs(1));
  assert_eq!(DEFAULT_MODEL, "jev-latest");
}

#[test]
fn a_transport_setting_toolu_http_refuses_is_a_transport_error() {
  let env = Env::from_pairs([("TYPESAFE_API_KEY", "k"), ("HTTPS_PROXY", "not a proxy")]);
  assert!(matches!(
    Jev::from_env(&env, Config::default()),
    Err(Error::Transport(toolu_http::Error::InvalidConfig(_)))
  ));
}

#[test]
fn a_node_extra_ca_certs_file_that_is_missing_or_holds_no_certificate_is_a_transport_error() {
  let dir = std::env::temp_dir().join(format!("toolu-jev-ca-{}", std::process::id()));
  std::fs::create_dir_all(&dir).unwrap();
  let empty = dir.join("empty.pem");
  std::fs::write(&empty, "no certificate here").unwrap();
  let absent = dir.join("absent.pem");
  for path in [empty, absent] {
    let env = Env::from_pairs([
      ("TYPESAFE_API_KEY", "k"),
      ("NODE_EXTRA_CA_CERTS", path.to_str().unwrap()),
    ]);
    assert!(matches!(
      Jev::from_env(&env, Config::default()),
      Err(Error::Transport(toolu_http::Error::InvalidConfig(_)))
    ));
  }
  std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn the_loopback_test_root_wins_over_node_extra_ca_certs() {
  let env = Env::from_pairs([
    ("TYPESAFE_API_KEY", "k"),
    ("NODE_EXTRA_CA_CERTS", "/nonexistent/ca.pem"),
  ]);
  let config = Config {
    test_root_ca_der: Some(vec![0, 1, 2]),
    ..Config::default()
  };
  // The invalid test root is what toolu-http refuses, not the missing PEM file.
  assert!(matches!(
    Jev::from_env(&env, config),
    Err(Error::Transport(toolu_http::Error::InvalidConfig(reason))) if reason.contains("test root")
  ));
}
