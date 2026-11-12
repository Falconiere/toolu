use std::fs;
use std::process::Command;

use toolu_runtime::env::Env;

use super::blob_ids;

#[test]
fn hashes_dirty_and_untracked_working_bytes() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  assert!(
    Command::new("git")
      .args(["init", "-q"])
      .current_dir(root)
      .status()
      .unwrap()
      .success()
  );
  fs::write(root.join("dirty.rs"), "pub fn changed() {}\n").unwrap();
  fs::write(root.join("untracked.rs"), "pub fn fresh() {}\n").unwrap();
  let paths = vec!["dirty.rs".to_owned(), "untracked.rs".to_owned()];
  let ids = blob_ids(&Env::process(), root, &paths).unwrap();
  assert_eq!(ids.len(), 2);
  for (path, id) in paths.iter().zip(ids) {
    let output = Command::new("git")
      .args(["hash-object", "--no-filters", path])
      .current_dir(root)
      .output()
      .unwrap();
    assert!(output.status.success());
    assert_eq!(id, String::from_utf8_lossy(&output.stdout).trim());
  }
}
