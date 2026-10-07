use std::fs;
use std::time::UNIX_EPOCH;

use super::{Record, append, file_for, retain, tail};

#[test]
fn append_is_ordered_and_a_partial_line_is_ignored() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  let lock = root.join("journal.lock");
  let journal = root.join("journal");
  let now = UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
  let first = append(
    &journal,
    &lock,
    &Record::new("action", "merge-intent", "a", "t1", "ok"),
    now,
  )
  .unwrap();
  assert_eq!(first.seq, 1);
  fs::write(file_for(&journal, now), "{\"seq\":1}\n{\"seq\":").unwrap();
  let second = append(
    &journal,
    &lock,
    &Record::new("action", "merge-done", "a", "t1", "ghp_secret"),
    now,
  )
  .unwrap();
  assert_eq!(second.note, "[redacted]");
  let rows = tail(&journal, now).unwrap();
  assert!(
    rows
      .iter()
      .any(|row| row.seq == second.seq && row.name == "merge-done")
  );
  retain(&journal, 0, now).unwrap();
}
