use std::fmt::Write as _;

use serde_json::{Value, json};

use super::document;
use crate::metrics::Query;

const DAY: &str = "2026-10-10";

fn journal(rows: &[(&str, &str, &str, &str, &str, u32)]) -> tempfile::TempDir {
  let dir = tempfile::tempdir().unwrap();
  let mut text = String::new();
  for (seq, (kind, name, epic, key, note, second)) in rows.iter().enumerate() {
    let line = json!({
      "seq": seq + 1, "at": format!("{DAY}T00:00:{second:02}.000Z"), "kind": kind,
      "name": name, "epic": epic, "key": key, "token": "", "note": note,
    });
    writeln!(text, "{line}").unwrap();
  }
  std::fs::write(dir.path().join(format!("{DAY}.jsonl")), text).unwrap();
  dir
}

fn query(epic: Option<&str>, since: Option<&str>) -> Query {
  Query {
    epic: epic.map(str::to_owned),
    since_ms: since.map(|since| crate::metrics::parse_since(since).unwrap()),
    now_ms: crate::metrics::parse_since("2026-10-10T00:01:00Z").unwrap(),
  }
}

fn rows() -> Vec<(
  &'static str,
  &'static str,
  &'static str,
  &'static str,
  &'static str,
  u32,
)> {
  vec![
    ("action", "launch-intent", "e1", "a", "", 0),
    ("transition", "phase", "e1", "a", "execution", 1),
    ("transition", "phase", "e1", "a", "ready", 11),
    ("action", "merge-intent", "e1", "a", "", 12),
    ("action", "merge-done", "e1", "a", "", 13),
    ("action", "cleanup-done", "e1", "a", "", 15),
    ("action", "launch-intent", "e2", "b", "", 20),
    ("transition", "phase", "e2", "b", "execution", 20),
    ("action", "wait-reply", "", "", "timeout", 30),
  ]
}

fn seconds(value: &Value) -> Value {
  json!(value.as_u64().map(|ms| ms / 1000))
}

#[test]
fn every_epic_by_default_and_one_epic_on_request() {
  let dir = journal(&rows());
  let all = document(dir.path(), &query(None, None)).unwrap();
  assert_eq!(all["records"], 9);
  assert_eq!(all["issues"], 2);
  assert_eq!(seconds(&all["lead_time_ms"]["p50"]), 15);
  assert_eq!(
    seconds(&all["phases"]["execution"]["ms"]),
    50,
    "b runs on to now"
  );
  assert_eq!(seconds(&all["ready_to_merged_ms"]["total"]["max"]), 2);
  assert_eq!(all["generated_at"], "2026-10-10T00:01:00.000Z");
  let one = document(dir.path(), &query(Some("e1"), None)).unwrap();
  assert_eq!(one["epic"], "e1");
  assert_eq!(one["records"], 7);
  assert_eq!(one["issues"], 1);
  assert_eq!(one["wakeups"]["judgment_free"], 1);
}

#[test]
fn since_drops_earlier_records_and_the_intervals_they_start() {
  let dir = journal(&rows());
  let doc = document(dir.path(), &query(None, Some("2026-10-10T00:00:14Z"))).unwrap();
  assert_eq!(doc["since"], "2026-10-10T00:00:14.000Z");
  assert_eq!(doc["records"], 4);
  assert_eq!(doc["lead_time_ms"]["count"], 0);
  assert_eq!(doc["issues"], 1);
}

#[test]
fn an_empty_journal_is_a_zero_document() {
  let dir = tempfile::tempdir().unwrap();
  let doc = document(&dir.path().join("journal"), &query(None, None)).unwrap();
  assert_eq!(doc["records"], 0);
  assert_eq!(
    doc["lead_time_ms"],
    json!({"count": 0, "p50": null, "p90": null, "max": null})
  );
  assert_eq!(doc["github"]["not_modified_share"], Value::Null);
  assert_eq!(doc["engine"]["max_rss_bytes"], Value::Null);
}
