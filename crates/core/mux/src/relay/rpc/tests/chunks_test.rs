use super::*;

#[test]
fn durable_chunks_reject_conflicts_and_bad_lengths() {
  let dir = tempfile::tempdir().unwrap();
  put(dir.path(), "task", 0, b"abc").unwrap();
  put(dir.path(), "task", 0, b"abc").unwrap();
  assert!(put(dir.path(), "task", 0, b"xyz").is_err());
  assert!(assemble(dir.path(), "task", 1, 4).is_err());
  assert_eq!(assemble(dir.path(), "task", 1, 3).unwrap(), b"abc");
}
