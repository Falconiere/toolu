use std::os::unix::fs::symlink;

use super::is_regular_file;

#[test]
fn a_regular_file_is_found_through_symlinks_and_nothing_else_is() {
  let dir = tempfile::tempdir().unwrap();
  let file = dir.path().join("a.toml");
  std::fs::write(&file, "").unwrap();
  symlink(&file, dir.path().join("link")).unwrap();
  symlink(dir.path().join("missing"), dir.path().join("dangling")).unwrap();
  std::fs::create_dir(dir.path().join("adir")).unwrap();
  assert!(is_regular_file(&file));
  assert!(is_regular_file(&dir.path().join("link")));
  assert!(!is_regular_file(&dir.path().join("dangling")));
  assert!(!is_regular_file(&dir.path().join("adir")));
  assert!(!is_regular_file(&dir.path().join("missing")));
}
