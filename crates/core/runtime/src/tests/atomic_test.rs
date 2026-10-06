use std::path::Path;

use super::write_atomic;

fn entries(dir: &Path) -> Vec<String> {
  let mut names: Vec<String> = std::fs::read_dir(dir)
    .unwrap()
    .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
    .collect();
  names.sort();
  names
}

#[test]
fn a_new_file_and_its_directory_are_created() {
  let dir = tempfile::tempdir().unwrap();
  let path = dir.path().join("a/b/state.json");
  assert!(write_atomic(&path, "{}\n"));
  assert_eq!(std::fs::read_to_string(&path).unwrap(), "{}\n");
  assert_eq!(entries(&dir.path().join("a/b")), ["state.json"]);
}

#[test]
fn a_symlink_at_the_target_is_replaced_not_written_through() {
  let dir = tempfile::tempdir().unwrap();
  let victim = dir.path().join("victim.txt");
  std::fs::write(&victim, "keep me").unwrap();
  let path = dir.path().join("state.json");
  std::os::unix::fs::symlink(&victim, &path).unwrap();
  assert!(write_atomic(&path, "new"));
  assert_eq!(std::fs::read_to_string(&victim).unwrap(), "keep me");
  assert!(
    std::fs::symlink_metadata(&path)
      .unwrap()
      .file_type()
      .is_file()
  );
  assert_eq!(std::fs::read_to_string(&path).unwrap(), "new");
}

#[test]
fn a_failed_write_reports_false_and_leaves_no_temp_file() {
  let dir = tempfile::tempdir().unwrap();
  std::fs::write(
    dir.path().join("blocker"),
    "a file where the directory goes",
  )
  .unwrap();
  assert!(!write_atomic(&dir.path().join("blocker/state.json"), "x"));
  let target_dir = dir.path().join("target");
  std::fs::create_dir(&target_dir).unwrap();
  std::fs::create_dir(target_dir.join("state.json")).unwrap();
  std::fs::write(target_dir.join("state.json/inside"), "x").unwrap();
  assert!(!write_atomic(&target_dir.join("state.json"), "x"));
  assert_eq!(entries(&target_dir), ["state.json"]);
}
