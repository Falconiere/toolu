use std::time::Duration;

use toolu_runtime::env::Env;

use crate::{Config, Error, Jev, timeout};

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
  for bad in ["-1", "soon", "inf", "1e400"] {
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
