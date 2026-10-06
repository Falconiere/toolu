use std::os::unix::fs::PermissionsExt as _;

use super::write_atomic;

fn names(dir: &std::path::Path) -> Vec<String> {
  let mut names: Vec<String> = std::fs::read_dir(dir)
    .unwrap()
    .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
    .collect();
  names.sort();
  names
}

#[test]
fn a_write_replaces_the_file_and_leaves_no_temp_behind() {
  let dir = tempfile::tempdir().unwrap();
  let file = dir.path().join("gate.json");
  std::fs::write(&file, "old\n").unwrap();
  assert!(write_atomic(&file, "new\n"));
  assert_eq!(std::fs::read_to_string(&file).unwrap(), "new\n");
  assert_eq!(names(dir.path()), ["gate.json"]);
}

#[test]
fn a_new_file_is_created_0600() {
  let dir = tempfile::tempdir().unwrap();
  let file = dir.path().join("gate.json");
  assert!(write_atomic(&file, "x"));
  let mode = std::fs::metadata(&file).unwrap().permissions().mode() & 0o777;
  assert_eq!(mode, 0o600);
}

#[test]
fn a_missing_directory_or_a_bare_name_fails_without_litter() {
  let dir = tempfile::tempdir().unwrap();
  assert!(!write_atomic(&dir.path().join("missing/gate.json"), "x"));
  assert_eq!(names(dir.path()), Vec::<String>::new());
  assert!(!write_atomic(std::path::Path::new("/"), "x"));
}

#[test]
fn a_directory_in_the_way_fails_and_cleans_up() {
  let dir = tempfile::tempdir().unwrap();
  let target = dir.path().join("gate.json");
  std::fs::create_dir(&target).unwrap();
  std::fs::write(target.join("keep"), "").unwrap();
  assert!(!write_atomic(&target, "x"));
  assert_eq!(names(dir.path()), ["gate.json"]);
}
