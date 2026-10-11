use std::time::{Duration, UNIX_EPOCH};

use toolu_runtime::config::secrets::Secrets;

use super::page;
use crate::journal::{Record, append};

#[test]
fn status_journal_page_is_ordered_and_omits_tokens() {
  let root = tempfile::tempdir().expect("temp root");
  let dir = root.path().join("journal");
  let lock = root.path().join("journal.lock");
  let first_day = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
  let later_day = first_day + Duration::from_hours(72);
  let first = append(
    &dir,
    &lock,
    &Record::new("transition", "brainstorm", "a", "private-token-a", "first"),
    first_day,
  )
  .expect("first event");
  let _later = append(
    &dir,
    &lock,
    &Record::new(
      "transition",
      "execution",
      "a",
      "private-token-b",
      &"x".repeat(250),
    ),
    later_day,
  )
  .expect("later event");
  let doc = page(&dir, first.seq, 100, &Secrets::none()).expect("event page");
  let events = doc["events"].as_array().expect("events");
  assert_eq!(events.len(), 1);
  assert_eq!(events[0]["seq"], first.seq + 1);
  assert_eq!(events[0]["name"], "execution");
  assert_eq!(
    events[0]["note"].as_str().expect("note").chars().count(),
    160
  );
  assert!(!doc.to_string().contains("private-token"));
  assert_eq!(doc["oldest_seq"], first.seq);
}
