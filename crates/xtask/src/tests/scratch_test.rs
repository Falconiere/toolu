use std::os::unix::fs::PermissionsExt as _;

use super::{create, make};

#[test]
fn each_call_makes_a_new_private_directory() {
  let first = create("toolu-scratch-test").unwrap();
  let second = create("toolu-scratch-test").unwrap();
  assert_ne!(first, second);
  let mode = std::fs::metadata(&first).unwrap().permissions().mode();
  assert_eq!(mode & 0o777, 0o700);
  assert!(first.starts_with(std::env::temp_dir()));
  std::fs::remove_dir(&first).unwrap();
  std::fs::remove_dir(&second).unwrap();
}

#[test]
fn anything_already_at_the_path_is_an_error() {
  let dir = tempfile::tempdir().unwrap();
  let err = make(dir.path()).unwrap_err();
  assert!(err.contains("cannot create"), "{err}");
  let file = dir.path().join("planted");
  std::fs::write(&file, "x").unwrap();
  assert!(make(&file).is_err());
  let fresh = dir.path().join("fresh");
  assert_eq!(make(&fresh).unwrap(), fresh);
  assert!(make(&dir.path().join("no/parent")).is_err());
}
