use serde_json::json;

use super::{Findings, at, includes, items, lists, runs, step_where, text};

#[test]
fn missing_and_mistyped_values_read_as_empty() {
  let doc = json!({"a": {"b": "c", "list": ["x"]}, "n": 1});
  assert_eq!(text(at(&doc, &["a", "b"])), "c");
  assert!(at(&doc, &["a", "missing"]).is_null());
  assert!(at(&doc, &["n", "deeper"]).is_null());
  assert_eq!(text(at(&doc, &["n"])), "");
  assert_eq!(items(at(&doc, &["a"])).len(), 0);
  assert!(lists(at(&doc, &["a", "list"]), "x"));
  assert!(!lists(at(&doc, &["a", "b"]), "c"));
  assert!(includes(at(&doc, &["a", "b"]), "c"));
}

#[test]
fn runs_join_the_step_scripts_and_a_step_is_found_by_its_fields() {
  let job =
    json!({"steps": [{"name": "one", "run": "a"}, {"uses": "x@v1"}, {"name": "two", "run": "b"}]});
  assert_eq!(runs(&job), "a\n\nb");
  assert_eq!(
    text(at(
      step_where(&job, |step| includes(at(step, &["name"]), "two")),
      &["run"]
    )),
    "b"
  );
  assert!(step_where(&job, |step| includes(at(step, &["name"]), "three")).is_null());
}

#[test]
fn need_records_only_what_does_not_hold() {
  let mut found = Findings::default();
  found.need(true, "kept out");
  found.need(false, "recorded");
  assert_eq!(found.0, ["recorded"]);
}
