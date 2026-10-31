use std::time::{Duration, UNIX_EPOCH};

use crate::journal::{Record, append, page};

#[test]
fn page_reads_the_record_after_the_cursor() {
  let dir = tempfile::tempdir().unwrap();
  let journal = dir.path().join("journal");
  let lock = dir.path().join("journal.lock");
  let now = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
  let first = append(
    &journal,
    &lock,
    &Record::new("action", "merge-intent", "k", "t", "n"),
    now,
  )
  .unwrap();
  append(
    &journal,
    &lock,
    &Record::new("action", "merge-done", "k", "t", "n"),
    now,
  )
  .unwrap();
  let rows = page(&journal, &lock, first.seq, 10).unwrap().1;
  assert_eq!(rows.len(), 1);
  assert_eq!(rows[0].seq, first.seq + 1);
}
