use serde_json::{Map, Value, json};

use super::{By, Rounds, count_round, note_phase, read, write};
use crate::model::Issue;

fn issue() -> Issue {
  Issue::blank("k", "e", "/state", 0)
}

#[test]
fn ready_joins_the_queue_once_and_any_other_phase_leaves_it() {
  let mut issue = issue();
  note_phase(&mut issue, "ready", 10);
  note_phase(&mut issue, "ready", 20);
  assert_eq!(issue.queued_at_ms, Some(10), "an earlier time is kept");
  note_phase(&mut issue, "babysit", 30);
  assert_eq!(issue.queued_at_ms, None);
  note_phase(&mut issue, "ready", 40);
  assert_eq!(issue.queued_at_ms, Some(40));
}

#[test]
fn rounds_count_by_who_and_number_every_round() {
  let mut issue = issue();
  assert_eq!(count_round(&mut issue, By::Engine), 1);
  assert_eq!(count_round(&mut issue, By::Worker), 2);
  assert_eq!(count_round(&mut issue, By::Engine), 3);
  assert_eq!(issue.rebase_rounds, Rounds { engine: 2, worker: 1 });
  assert_eq!((By::Engine.name(), By::Worker.name()), ("engine", "worker"));
}

#[test]
fn the_fields_round_trip_through_a_record() {
  let mut issue = issue();
  issue.queued_at_ms = Some(1_760_227_200_123);
  issue.rebase_rounds = Rounds { engine: 2, worker: 1 };
  let mut map = Map::new();
  write(&mut map, &issue);
  assert_eq!(map["queued_at"], json!("2025-10-12T00:00:00.123Z"));
  assert_eq!(map["rebase_rounds"], json!({"engine": 2, "worker": 1}));
  let mut back = Issue::blank("k", "e", "/state", 0);
  read(&mut back, &Value::Object(map.clone()));
  assert_eq!(back.queued_at_ms, issue.queued_at_ms);
  assert_eq!(back.rebase_rounds, issue.rebase_rounds);
  issue.queued_at_ms = None;
  issue.rebase_rounds = Rounds::default();
  write(&mut map, &issue);
  assert!(map.is_empty(), "{map:?}");
}

#[test]
fn a_record_without_or_with_bad_fields_reads_as_not_queued() {
  let mut issue = issue();
  issue.queued_at_ms = Some(5);
  read(&mut issue, &json!({"queued_at": "yesterday", "rebase_rounds": {"engine": -1}}));
  assert_eq!(issue.queued_at_ms, None);
  assert_eq!(issue.rebase_rounds, Rounds::default());
}
