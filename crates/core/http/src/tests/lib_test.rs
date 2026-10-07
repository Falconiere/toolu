use std::time::Duration;

use toolu_runtime::env::Env;

use crate::{Auth, Client, Config, Error, proxy_for};

#[test]
fn rejects_zero_timeout_before_request() {
  let config = Config {
    timeout: Duration::ZERO,
    ..Config::default()
  };
  let result = Client::new(config, &Env::default());
  assert!(matches!(result, Err(Error::InvalidConfig(_))));
}

#[test]
fn rejects_zero_body_limit_before_request() {
  let config = Config {
    max_body_bytes: 0,
    ..Config::default()
  };
  let result = Client::new(config, &Env::default());
  assert!(matches!(result, Err(Error::InvalidConfig(_))));
}

#[test]
fn scheme_proxy_precedes_all_proxy_and_supports_lowercase() {
  let env = Env::from_pairs([
    ("ALL_PROXY", "http://all.example:1"),
    ("https_proxy", "http://secure.example:2"),
    ("HTTP_PROXY", "http://plain.example:3"),
  ]);
  assert_eq!(proxy_for(&env, "https"), Some("http://secure.example:2"));
  assert_eq!(proxy_for(&env, "http"), Some("http://plain.example:3"));
  let fallback = Env::from_pairs([("all_proxy", "http://all.example:4")]);
  assert_eq!(proxy_for(&fallback, "https"), Some("http://all.example:4"));
  assert_eq!(proxy_for(&fallback, "http"), Some("http://all.example:4"));
}

#[test]
fn rejects_invalid_proxy_and_test_root() {
  let env = Env::from_pairs([("HTTPS_PROXY", "not a proxy")]);
  assert!(matches!(
    Client::new(Config::default(), &env),
    Err(Error::InvalidConfig(_))
  ));
  let config = Config {
    test_root_ca_der: Some(vec![0, 1, 2]),
    ..Config::default()
  };
  assert!(matches!(
    Client::new(config, &Env::default()),
    Err(Error::InvalidConfig(_))
  ));
}

#[test]
fn rejects_malformed_or_non_http_urls_before_network_access() {
  let client = Client::new(Config::default(), &Env::default()).expect("client");
  for url in ["https://[broken", "ftp://example.test/data"] {
    assert!(matches!(
      client.get_bytes(url, &Auth::None),
      Err(Error::Transport(_))
    ));
  }
}

#[test]
fn debug_shows_the_config_but_no_environment_value() {
  let env = Env::from_pairs([("HTTPS_PROXY", "http://user:proxypass@127.0.0.1:1")]);
  let client = Client::new(Config::default(), &env).expect("client");
  let shown = format!("{client:?}");
  assert!(shown.starts_with("Client { config: Config {"), "{shown}");
  assert!(!shown.contains("proxypass"), "{shown}");
}
