use std::path::Path;

use super::load;

const REPO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

#[test]
fn the_repository_members_and_their_targets_are_read() {
  let metadata = load(Path::new(REPO)).unwrap();
  let names: Vec<&str> = metadata
    .members()
    .map(|package| package.name.as_str())
    .collect();
  assert!(
    names.contains(&"xtask") && names.contains(&"toolu-protocol"),
    "{names:?}"
  );
  let xtask = metadata
    .members()
    .find(|package| package.name == "xtask")
    .unwrap();
  assert!(xtask.has_target("bin") && !xtask.has_target("lib"));
  assert!(xtask.dir().ends_with("crates/xtask"));
  let dev = xtask
    .dependencies
    .iter()
    .find(|dep| dep.name == "tempfile")
    .unwrap();
  assert!(!dev.is_linked());
  assert!(
    xtask
      .dependencies
      .iter()
      .find(|dep| dep.name == "syn")
      .unwrap()
      .is_linked()
  );
}

#[test]
fn a_directory_without_a_workspace_is_an_error() {
  let dir = tempfile::tempdir().unwrap();
  let err = load(dir.path()).unwrap_err();
  assert!(err.starts_with("cargo metadata failed"), "{err}");
}
