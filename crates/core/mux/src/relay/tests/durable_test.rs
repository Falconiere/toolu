use super::{create, ensure_dir, write};

#[test]
fn creates_nested_directories_and_preserves_existing_marker() {
  let dir = tempfile::tempdir().unwrap();
  let nested = dir.path().join("a/b");
  ensure_dir(&nested).unwrap();
  let marker = nested.join("intent");
  create(&marker, b"first").unwrap();
  assert!(create(&marker, b"second").is_err());
  assert_eq!(std::fs::read(marker).unwrap(), b"first");
}

#[test]
fn failed_replacement_preserves_existing_file() {
  let dir = tempfile::tempdir().unwrap();
  let file = dir.path().join("file");
  write(&file, b"original").unwrap();
  assert!(write(&file.join("child"), b"replacement").is_err());
  assert_eq!(std::fs::read(file).unwrap(), b"original");
}
