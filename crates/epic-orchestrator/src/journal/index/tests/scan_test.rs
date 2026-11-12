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

#[test]
fn truncated_log_rebuilds_before_page_or_append() {
  let (root, dir, lock, _at) = truncated_log();
  assert_eq!(
    crate::journal::page(&dir, &lock, 0, 10)
      .unwrap()
      .1
      .iter()
      .map(|row| row.seq)
      .collect::<Vec<_>>(),
    [1]
  );
  drop(root);

  let (_root, dir, lock, at) = truncated_log();
  let stored = crate::journal::append(
    &dir,
    &lock,
    &crate::journal::Record::new("action", "replacement", "k", "t", "n"),
    at,
  )
  .unwrap();
  assert_eq!(stored.seq, 2);
  assert_eq!(
    crate::journal::page(&dir, &lock, 0, 10)
      .unwrap()
      .1
      .iter()
      .map(|row| row.seq)
      .collect::<Vec<_>>(),
    [1, 2]
  );
}

fn truncated_log() -> (
  tempfile::TempDir,
  std::path::PathBuf,
  std::path::PathBuf,
  std::time::SystemTime,
) {
  let root = tempfile::tempdir().unwrap();
  let dir = root.path().join("journal");
  let lock = root.path().join("journal.lock");
  let at = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
  let first = crate::journal::Record::new("action", "first", "k", "t", "n");
  crate::journal::append(&dir, &lock, &first, at).unwrap();
  let path = crate::journal::file_for(&dir, at);
  let previous_len = std::fs::metadata(&path).unwrap().len();
  let second = crate::journal::Record::new("action", "second", "k", "t", "n");
  crate::journal::append(&dir, &lock, &second, at).unwrap();
  std::fs::OpenOptions::new()
    .write(true)
    .open(path)
    .unwrap()
    .set_len(previous_len)
    .unwrap();
  (root, dir, lock, at)
}
