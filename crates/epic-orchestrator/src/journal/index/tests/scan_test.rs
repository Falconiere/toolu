use std::time::{Duration, UNIX_EPOCH};

use crate::journal::{Record, append, page};

#[test]
fn deleting_the_index_rebuilds_from_the_log() {
  let dir = tempfile::tempdir().unwrap();
  let journal = dir.path().join("journal");
  let lock = dir.path().join("journal.lock");
  let now = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
  append(
    &journal,
    &lock,
    &Record::new("action", "merge-intent", "k", "t", "n"),
    now,
  )
  .unwrap();
  std::fs::remove_file(journal.join("sequence.index")).unwrap();
  std::fs::remove_file(journal.join("sequence.offsets")).unwrap();
  let rows = page(&journal, &lock, 0, 10).unwrap().1;
  assert_eq!(rows.len(), 1);
  assert_eq!(rows[0].seq, 1);
}
