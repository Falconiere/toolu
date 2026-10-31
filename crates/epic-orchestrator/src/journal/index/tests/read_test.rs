#[test]
fn the_second_record_is_the_page_after_the_first_sequence() {
  let root = tempfile::tempdir().unwrap();
  let dir = root.path().join("tail");
  let lock = root.path().join("held");
  let at = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_500_000_000);
  let row = crate::journal::Record::new("action", "read", "k", "t", "n");
  let first = crate::journal::append(&dir, &lock, &row, at).unwrap();
  crate::journal::append(&dir, &lock, &row, at).unwrap();
  let rows = crate::journal::page(&dir, &lock, first.seq, 5).unwrap().1;
  assert_eq!(rows.len(), 1);
  assert_eq!(rows[0].name, "read");
}
