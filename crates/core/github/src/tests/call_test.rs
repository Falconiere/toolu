use std::thread;

use toolu_runtime::env::Env;

use crate::{Client, Config, Error};

fn client(api_url: &str) -> Client {
  let config = Config {
    api_url: api_url.to_owned(),
    ..Config::scheduled()
  };
  Client::new(config, &Env::from_pairs([("GH_TOKEN", "t-460")])).expect("client")
}

#[test]
fn a_path_is_appended_and_a_link_under_the_api_kept() {
  let client = client("https://api.github.com/");
  assert_eq!(
    client.url("/repos/o/r/pulls/1").expect("url"),
    "https://api.github.com/repos/o/r/pulls/1"
  );
  assert_eq!(
    client
      .url("https://api.github.com/repos/o/r/pulls?page=2")
      .expect("link"),
    "https://api.github.com/repos/o/r/pulls?page=2"
  );
}

#[test]
fn a_path_off_the_api_is_refused() {
  let client = client("https://api.github.com");
  for path in [
    "https://api.github.com.evil.example/x",
    "//evil.example/x",
    "repos/o/r",
    "https://api.github.com",
  ] {
    assert_eq!(
      client.url(path),
      Err(Error::Config(format!(
        "{path} is not a path under https://api.github.com"
      )))
    );
  }
}

#[test]
fn an_error_message_is_read_from_json_and_redacted() {
  let client = client("https://api.github.com");
  assert_eq!(
    client.message(br#"{"message":"bad token t-460"}"#),
    "bad token <redacted>"
  );
  assert_eq!(client.message(b"<html>"), "");
  assert_eq!(client.message(br#"{"message":3}"#), "");
}

#[test]
fn a_path_with_spaces_controls_or_non_ascii_is_refused() {
  let client = client("https://api.github.com");
  for path in ["/repos/o/r/contents/a b", "/x\ny", "/caf\u{e9}"] {
    assert_eq!(
      client.url(path),
      Err(Error::Config(format!(
        "{path:?} is not a URL path: percent-encode spaces, controls and non-ASCII"
      )))
    );
  }
}

#[test]
fn a_poisoned_token_lock_is_recovered() {
  let client = client("https://api.github.com");
  let poisoned = thread::scope(|scope| {
    scope
      .spawn(|| {
        let _held = client.tokens();
        panic!("poison the token lock");
      })
      .join()
  });
  assert!(poisoned.is_err());
  assert_eq!(client.tokens().current().expose(), "t-460");
}
