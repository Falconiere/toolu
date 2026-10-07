use std::time::Duration;

use toolu_runtime::env::Env;

use crate::{API_URL, Client, Config, Error, Retry};

fn env() -> Env {
  Env::from_pairs([("GH_TOKEN", "secret-460")])
}

#[test]
fn the_configs_target_the_api_with_their_policy() {
  let one_shot = Config::one_shot(&Env::default()).expect("one-shot");
  assert_eq!(one_shot.api_url, API_URL);
  assert_eq!(one_shot.retry.attempts, 3);
  assert_eq!(one_shot.http.timeout, Duration::from_secs(60));
  assert_eq!(one_shot.http.max_body_bytes, 8 * 1024 * 1024);
  let scheduled = Config::scheduled();
  assert_eq!(scheduled.retry, Retry::scheduled());
  assert_eq!(scheduled.http.timeout, Duration::from_secs(30));
}

#[test]
fn the_api_url_must_be_an_https_origin() {
  for api_url in [
    "http://api.example.test",
    "https://",
    "https://api.example.test/v3",
    "ftp://x",
    "https://bad host",
  ] {
    let config = Config {
      api_url: api_url.to_owned(),
      ..Config::scheduled()
    };
    assert_eq!(
      Client::new(config, &env()).map(|_| ()),
      Err(Error::Config(format!(
        "the API URL must be an https:// origin, not {}",
        api_url.trim_end_matches('/')
      )))
    );
  }
}

#[test]
fn a_policy_without_attempts_is_refused() {
  let config = Config {
    retry: Retry {
      attempts: 0,
      backoff: Vec::new(),
      max_wait: Duration::ZERO,
    },
    ..Config::scheduled()
  };
  assert_eq!(
    Client::new(config, &env()).map(|_| ()),
    Err(Error::Config(
      "the retry policy needs at least 1 attempt".into()
    ))
  );
}

#[test]
fn debug_shows_the_api_and_policy_but_no_token() {
  let client = Client::new(Config::scheduled(), &env()).expect("client");
  let shown = format!("{client:?}");
  assert!(
    shown.starts_with("Client { api_url: \"https://api.github.com\""),
    "{shown}"
  );
  assert!(!shown.contains("secret-460"), "{shown}");
}
