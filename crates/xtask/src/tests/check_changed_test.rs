use std::path::{Path, PathBuf};

use super::{packages, run};
use crate::metadata;
use crate::options::Options;

const REPO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

#[test]
fn changed_paths_select_the_deepest_crate_or_workspace() {
  let metadata = metadata::load(Path::new(REPO)).unwrap();
  assert_eq!(
    packages(&metadata, &[PathBuf::from("crates/xtask/src/gate.rs")]),
    Some(vec!["xtask".to_owned()])
  );
  assert_eq!(packages(&metadata, &[PathBuf::from("Cargo.lock")]), None);
}

#[test]
fn protocol_change_includes_linked_reverse_dependents() {
  let metadata = metadata::load(Path::new(REPO)).unwrap();
  let selected = packages(
    &metadata,
    &[PathBuf::from("crates/core/protocol/src/hook.rs")],
  )
  .unwrap();
  for name in ["toolu-protocol", "toolu-cli", "xtask"] {
    assert!(
      selected.contains(&name.to_owned()),
      "{name} missing: {selected:?}"
    );
  }
}

#[test]
fn invalid_base_is_a_setup_error() {
  let options = Options {
    root: PathBuf::from(REPO),
    base: Some("no-such-ref".to_owned()),
    ..Options::default()
  };
  let err = run(&options).unwrap_err();
  assert!(
    err.starts_with("git merge-base no-such-ref HEAD failed"),
    "{err}"
  );
}

#[test]
fn documentation_does_not_expand_the_rust_test_scope() {
  let metadata = metadata::load(Path::new(REPO)).unwrap();
  let docs = [
    "AGENTS.md",
    "README.md",
    "docs/testing.md",
    "docs/cli/gate.md",
  ];
  let mut paths: Vec<_> = docs.iter().map(PathBuf::from).collect();
  assert_eq!(packages(&metadata, &paths), Some(Vec::new()));
  paths.push(PathBuf::from("crates/xtask/src/gate.rs"));
  assert_eq!(packages(&metadata, &paths), Some(vec!["xtask".to_owned()]));
  paths.push(PathBuf::from("docs/cli/commands.json"));
  assert_eq!(packages(&metadata, &paths), None);
}

#[test]
fn test_support_changes_include_dev_dependency_consumers() {
  let metadata = metadata::load(Path::new(REPO)).unwrap();
  let selected = packages(
    &metadata,
    &[PathBuf::from("crates/http-test-support/src/lib.rs")],
  )
  .unwrap();
  for name in ["toolu-http-test-support", "toolu-http", "toolu-cli"] {
    assert!(
      selected.contains(&name.to_owned()),
      "{name} missing: {selected:?}"
    );
  }
}
