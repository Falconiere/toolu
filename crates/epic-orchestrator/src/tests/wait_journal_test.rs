use std::time::SystemTime;

use serde_json::json;

use super::replied;
use crate::journal;
use crate::server::{Engine, Fault};

#[test]
fn a_judgment_reply_and_a_timeout_are_journaled_as_wake_ups() {
  let dir = tempfile::tempdir().unwrap();
  let engine = Engine::open(crate::paths::Paths::at(dir.path()), None, Fault::None).unwrap();
  replied(
    &engine,
    &json!({"kind": "needs-human", "key": "i-1", "epic": "e"}),
  )
  .unwrap();
  replied(&engine, &json!({"state": "waiting"})).unwrap();
  let rows = journal::tail(&engine.paths.journal_dir(), SystemTime::now()).unwrap();
  let replies: Vec<(&str, &str, &str)> = rows
    .iter()
    .filter(|row| row.name == "wait-reply")
    .map(|row| (row.key.as_str(), row.epic.as_str(), row.note.as_str()))
    .collect();
  assert_eq!(
    replies,
    [("i-1", "e", "judgment:needs-human"), ("", "", "timeout")]
  );
}
