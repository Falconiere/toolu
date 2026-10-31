#[test]
fn rebuild_keeps_seq_one_after_the_cache_files_are_removed() {
  let root = tempfile::tempdir().unwrap();
  let dir = root.path().join("days");
  let lock = root.path().join("gate");
  let at = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_600_000_000);
  let row = crate::journal::Record::new("action", "scan", "k", "t", "n");
  crate::journal::append(&dir, &lock, &row, at).unwrap();
  std::fs::remove_file(dir.join("sequence.index")).unwrap();
  std::fs::remove_file(dir.join("sequence.offsets")).unwrap();
  assert_eq!(crate::journal::page(&dir, &lock, 0, 3).unwrap().1[0].seq, 1);
}
