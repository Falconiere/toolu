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
    assert!(selected.contains(&name.to_owned()), "{name} missing: {selected:?}");
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
  assert!(err.starts_with("git merge-base no-such-ref HEAD failed"), "{err}");
}
