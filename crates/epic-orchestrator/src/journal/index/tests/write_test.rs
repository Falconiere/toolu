#[test]
fn the_synced_record_is_a_single_terminated_line() {
  let root = tempfile::tempdir().unwrap();
  let dir = root.path().join("out");
  let lock = root.path().join("mu");
  let at = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_400_000_000);
  let row = crate::journal::Record::new("action", "write", "k", "t", "n");
  let stored = crate::journal::append(&dir, &lock, &row, at).unwrap();
  let text = std::fs::read_to_string(crate::journal::file_for(&dir, at)).unwrap();
  assert_eq!(text.chars().filter(|ch| *ch == '\n').count(), 1);
  assert!(text.contains(&format!("\"seq\":{}", stored.seq)));
}
