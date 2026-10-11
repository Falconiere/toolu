use toolu_runtime::env::Env;

use super::{Jev, jev};

#[test]
fn missing_key_is_named_without_exposing_an_environment_value() {
  let env = Env::from_pairs([("PATH", "/no-such-directory")]);
  assert_eq!(
    jev(&env),
    Jev::Unavailable("missing toolu jev; missing TYPESAFE_API_KEY".to_owned())
  );
}

#[test]
fn empty_key_is_unavailable() {
  let env = Env::from_pairs([("PATH", "/no-such-directory"), ("TYPESAFE_API_KEY", "")]);
  assert_eq!(
    jev(&env),
    Jev::Unavailable("missing toolu jev; missing TYPESAFE_API_KEY".to_owned())
  );
}

#[test]
fn a_multiline_key_is_rejected_without_printing_it() {
  let env = Env::from_pairs([
    ("PATH", "/no-such-directory"),
    ("TYPESAFE_API_KEY", "secret\nvalue"),
  ]);
  let Jev::Unavailable(reason) = jev(&env) else {
    panic!("expected unavailable");
  };
  assert!(reason.contains("invalid TYPESAFE_API_KEY"));
  assert!(!reason.contains("secret"));
}
