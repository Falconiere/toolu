use super::snapshot;

#[test]
fn a_path_without_git_is_not_snapshotted() {
  let tmp = tempfile::tempdir().expect("temp");
  let path = tmp.path().join("missing");
  std::fs::create_dir(&path).expect("dir");
  snapshot(&path.display().to_string(), "a", None).expect("snapshot");
  assert!(!path.join(".git").exists());
}
