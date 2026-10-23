use std::time::SystemTime;

use serde_json::json;

use super::replied;
use crate::journal::{self, Record};
use crate::server::{Engine, Fault};

#[test]
fn a_judgment_reply_and_a_timeout_are_journaled_as_wake_ups() {
  let dir = tempfile::tempdir().unwrap();
  let engine = Engine::open(crate::paths::Paths::at(dir.path()), None, Fault::None).unwrap();
  replied(
    &engine,
    &json!({"kind": "needs-human", "key": "i-1", "epic": "e"}),
    "a7-3",
  )
  .unwrap();
  replied(&engine, &json!({"state": "waiting"}), "").unwrap();
  let rows = journal::tail(&engine.paths.journal_dir(), SystemTime::now()).unwrap();
  let replies: Vec<(&str, &str, &str, &str)> = rows
    .iter()
    .filter(|row| row.name == "wait-reply")
    .map(|row| {
      let (key, epic) = (row.key.as_str(), row.epic.as_str());
      (key, epic, row.token.as_str(), row.note.as_str())
    })
    .collect();
  assert_eq!(
    replies,
    [
      ("i-1", "e", "a7-3", "judgment:needs-human"),
      ("", "", "", "timeout")
    ]
  );
}

#[test]
fn the_rebuild_marks_a_replied_item_delivered_until_it_reopens() {
  let dir = tempfile::tempdir().unwrap();
  let paths = crate::paths::Paths::at(dir.path());
  let now = SystemTime::now();
  let rows = [
    Record::new("judgment", "failed", "i-1", "a1-1", "boom"),
    Record::new("judgment", "failed", "i-2", "a2-2", "boom"),
    Record::new("judgment", "stall", "i-3", "a3-3", "stalled"),
    Record::new("action", "wait-reply", "i-1", "a1-1", "judgment:failed"),
    Record::new("action", "wait-reply", "i-3", "a3-3", "judgment:stall"),
    Record::new("judgment", "reopened", "i-3", "a3-3", "stalled again"),
    Record::new("action", "wait-reply", "", "", "timeout"),
  ];
  for row in &rows {
    journal::append(&paths.journal_dir(), &paths.journal_lock(), row, now).unwrap();
  }
  let world = crate::snapshot::load(&paths, now).unwrap();
  let delivered: Vec<(&str, bool)> = world
    .attention
    .iter()
    .map(|item| (item.key.as_str(), item.delivered))
    .collect();
  assert_eq!(delivered, [("i-1", true), ("i-2", false), ("i-3", false)]);
}
