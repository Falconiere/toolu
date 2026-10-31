use std::time::{Duration, UNIX_EPOCH};

use crate::journal::{Record, append, file_for};

#[test]
fn a_synced_line_ends_with_a_newline() {
  let dir = tempfile::tempdir().unwrap();
  let journal = dir.path().join("journal");
  let lock = dir.path().join("journal.lock");
  let now = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
  let stored = append(
    &journal,
    &lock,
    &Record::new("action", "merge-intent", "k", "t", "n"),
    now,
  )
  .unwrap();
  let text = std::fs::read_to_string(file_for(&journal, now)).unwrap();
  assert!(text.ends_with('\n'));
  assert!(text.contains(&format!("\"seq\":{}", stored.seq)));
}
