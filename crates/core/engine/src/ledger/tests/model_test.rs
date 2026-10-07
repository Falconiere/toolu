//! Recompute, the summary line and orphan healing, against the values the
//! TypeScript `ledger-model.ts` computes for the same ledgers.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use toolu_runtime::json::jq_text;
use toolu_runtime::json::ordered::Ordered;

use super::{
  all_fresh, entries_by_id, heal_orphans, orphan_cutoff, recompute, steps_with_id, summary_line,
};
use crate::ledger::jq::{parse_json, string};

fn doc(text: &str) -> Ordered {
  parse_json(text).unwrap()
}

fn text(value: &Ordered) -> String {
  jq_text(value, false)
}

const LEDGER: &str = r#"{"version":1,"branch":"feat/x","steps":[
{"id":"s1","status":"green","diff_sha":"A","scope_sha":"S1","model":null},
{"id":"s2","status":"green","diff_sha":"B","model":"opus"},
{"id":"s3","status":"red","diff_sha":"A","retries":[{"attempt":1}],"model":"haiku"},
{"id":"s4","status":"pending"},
{"id":"s5","status":"running","started_at":"2031-01-01T00:00:00Z"}]}"#;

const SUMMARY: &str = r#""summary":{"total":5,"green":2,"red":1,"pending":1,"running":1,"stale":1,"fresh_green":1,"retried":1}"#;

fn empty() -> Ordered {
  Ordered::Object(Vec::new())
}

#[test]
fn recompute_counts_and_names_the_first_stale_step() {
  let ledger = doc(LEDGER);
  let recomputed = recompute(&ledger, "A", &empty(), false).unwrap();
  let shown = text(&recomputed);
  assert!(
    shown.ends_with(&format!(r#"}}],{SUMMARY},"next":"s2"}}"#)),
    "{shown}"
  );
  let scope = doc(r#"{"s1":"S1","s2":"X"}"#);
  assert_eq!(
    text(&recompute(&ledger, "A", &scope, false).unwrap()),
    shown
  );
  let verify = recompute(&ledger, "A", &doc(r#"{"s1":"S1"}"#), true).unwrap();
  assert_eq!(text(&verify), shown);
  assert!(!all_fresh(&recomputed));
}

#[test]
fn a_scope_hash_decides_freshness_unless_verifying() {
  let ledger = doc(r#"{"steps":[{"id":"s1","status":"green","diff_sha":"OLD","scope_sha":"S1"}]}"#);
  let scope = doc(r#"{"s1":"S1"}"#);
  let scoped = recompute(&ledger, "NEW", &scope, false).unwrap();
  assert_eq!(scoped.get("next"), Some(&Ordered::Null));
  assert!(all_fresh(&scoped));
  let verified = recompute(&ledger, "NEW", &scope, true).unwrap();
  assert_eq!(verified.get("next"), Some(&string("s1")));
}

#[test]
fn recompute_fails_where_jq_fails() {
  assert!(recompute(&doc(r#"{"steps":3}"#), "A", &empty(), false).is_err());
  let numeric = doc(r#"{"steps":[{"id":3,"status":"green","diff_sha":"A"}]}"#);
  assert!(recompute(&numeric, "A", &empty(), false).is_err());
  assert_eq!(
    text(&recompute(&numeric, "A", &empty(), true).unwrap()),
    r#"{"steps":[{"id":3,"status":"green","diff_sha":"A"}],"summary":{"total":1,"green":1,"red":0,"pending":0,"running":0,"stale":0,"fresh_green":1,"retried":0},"next":null}"#
  );
  let object = doc(r#"{"steps":{"a":{"id":"a","status":"green","diff_sha":"A"}}}"#);
  let recomputed = recompute(&object, "A", &empty(), false).unwrap();
  assert_eq!(recomputed.get("next"), Some(&Ordered::Null));
}

#[test]
fn the_summary_line_names_the_next_step_and_its_model() {
  let ledger = doc(LEDGER);
  let plain = recompute(&ledger, "A", &empty(), false).unwrap();
  assert_eq!(
    summary_line(&plain, "feat_x").unwrap(),
    "plan-ledger feat_x: 1/5 fresh-green, next=s2 model=opus"
  );
  let scoped = recompute(&ledger, "B", &doc(r#"{"s1":"S1"}"#), false).unwrap();
  assert_eq!(
    summary_line(&scoped, "feat_x").unwrap(),
    "plan-ledger feat_x: 2/5 fresh-green, next=s3 model=haiku"
  );
  let done = doc(r#"{"next":null,"summary":{"fresh_green":1,"total":1},"steps":[{"id":"a"}]}"#);
  assert_eq!(
    summary_line(&done, "x").unwrap(),
    "plan-ledger x: 1/1 fresh-green, next=none"
  );
  let false_model =
    doc(r#"{"next":"a","summary":{"fresh_green":0,"total":1},"steps":[{"id":"a","model":false}]}"#);
  assert!(summary_line(&false_model, "x").is_err());
  let numeric_next = doc(r#"{"next":3,"summary":{"fresh_green":0,"total":1},"steps":[{"id":3}]}"#);
  assert!(summary_line(&numeric_next, "x").is_err());
}

#[test]
fn healing_returns_old_or_unstamped_running_steps_to_pending() {
  let ledger = doc(
    r#"{"steps":[
{"id":"a","status":"running","started_at":"2031-01-01T00:00:00Z","activity":"x"},
{"id":"b","status":"running","started_at":"2031-06-01T00:00:00Z"},
{"id":"c","status":"running","started_at":""},
{"id":"d","status":"running","started_at":null},
{"id":"e","status":"running","started_at":5},
{"id":"f","status":"running","started_at":[1]},
{"id":"g","status":"green"}]}"#,
  );
  let healed = heal_orphans(&ledger, "2031-03-01T00:00:00Z").unwrap();
  assert_eq!(
    text(&healed),
    r#"{"steps":[{"id":"a","status":"pending","started_at":null,"activity":null},{"id":"b","status":"running","started_at":"2031-06-01T00:00:00Z"},{"id":"c","status":"pending","started_at":null,"activity":null},{"id":"d","status":"pending","started_at":null,"activity":null},{"id":"e","status":"pending","started_at":null,"activity":null},{"id":"f","status":"running","started_at":[1]},{"id":"g","status":"green"}]}"#
  );
  assert!(heal_orphans(&doc(r#"{"steps":null}"#), "x").is_err());
}

#[test]
fn the_orphan_cutoff_floors_to_the_second() {
  let at = |millis: u64| UNIX_EPOCH + Duration::from_millis(millis);
  // 2031-03-01T00:05:00.900Z
  let now: SystemTime = at(1_930_089_900_900);
  assert_eq!(orphan_cutoff(now, 300), "2031-03-01T00:00:00Z");
  assert_eq!(orphan_cutoff(now, -60), "2031-03-01T00:06:00Z");
  assert_eq!(orphan_cutoff(UNIX_EPOCH, 60), "1969-12-31T23:59:00Z");
}

#[test]
fn entries_are_indexed_by_string_id_and_matched_with_duplicates() {
  let ledger = doc(r#"{"steps":[{"id":"a","n":1},{"id":"b"},{"id":"a","n":2}]}"#);
  let entries = entries_by_id(&ledger).unwrap();
  assert_eq!(
    entries.get("a").map(text).as_deref(),
    Some(r#"{"id":"a","n":2}"#)
  );
  assert!(entries_by_id(&doc(r#"{"steps":[{"id":1}]}"#)).is_err());
  let steps = vec![
    doc(r#"{"id":"a"}"#),
    doc(r#"{"id":"b"}"#),
    doc(r#"{"id":"a","x":1}"#),
  ];
  assert_eq!(steps_with_id(&steps, &string("a")).unwrap().len(), 2);
  assert!(steps_with_id(&[doc("3")], &string("a")).is_err());
}
