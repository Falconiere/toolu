use std::path::Path;

use super::{check, problem};

#[test]
fn the_binary_protocol_passes_and_anything_else_is_named() {
  assert_eq!(problem(r#"{"hookProtocol":1}"#), None);
  assert_eq!(
    problem(r#"{"hookProtocol":2}"#).as_deref(),
    Some("hookProtocol is 2, but toolu speaks 1")
  );
  for text in [
    r#"{"version":"1.0.0"}"#,
    r#"{"hookProtocol":"1"}"#,
    r#"{"hookProtocol":1.5}"#,
  ] {
    assert!(
      problem(text)
        .unwrap()
        .starts_with("no integer hookProtocol"),
      "{text}"
    );
  }
  assert!(problem("{").unwrap().starts_with("invalid JSON"));
}

#[test]
fn every_real_manifest_declares_the_protocol() {
  let root = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
  for plugin in ["toolu", "jev", "rust-quality"] {
    assert_eq!(check(root, plugin), Vec::new(), "{plugin}");
  }
}
