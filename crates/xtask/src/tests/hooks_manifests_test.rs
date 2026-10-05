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

#[test]
fn an_unreadable_manifest_is_a_finding_and_an_absent_one_is_not() {
  let dir = tempfile::tempdir().unwrap();
  std::fs::create_dir_all(dir.path().join("plugins/x/.claude-plugin/plugin.json")).unwrap();
  let found = check(dir.path(), "x");
  assert_eq!(found.len(), 1, "{found:?}");
  assert_eq!(found[0].file, "plugins/x/.claude-plugin/plugin.json");
  assert!(found[0].problem.starts_with("cannot read: "), "{found:?}");
}
