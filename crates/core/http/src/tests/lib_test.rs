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
fn basic_auth_keeps_empty_password() {
  let value = Auth::Basic {
    username: "alice".into(),
    password: String::new(),
  }
  .header_value();
  assert_eq!(value.as_deref(), Some("Basic YWxpY2U6"));
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
fn errors_have_actionable_messages() {
  let cases = [
    (
      Error::InvalidConfig("timeout".into()),
      "invalid HTTP configuration: timeout",
    ),
    (Error::HttpStatus(429), "HTTP status 429"),
    (Error::Timeout, "HTTP request timed out"),
    (Error::BodyTooLarge, "HTTP response body exceeds limit"),
    (
      Error::Encode("bad value".into()),
      "cannot encode JSON request: bad value",
    ),
    (
      Error::Decode("bad JSON".into()),
      "cannot decode JSON response: bad JSON",
    ),
    (
      Error::Transport("offline".into()),
      "HTTP transport error: offline",
    ),
  ];
  for (error, expected) in cases {
    assert_eq!(error.to_string(), expected);
  }
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
