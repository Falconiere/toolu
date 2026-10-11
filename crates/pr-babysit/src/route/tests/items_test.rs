use serde_json::{Value, json};

use super::{heuristic, parse};
use crate::fixture;

fn message(document: &Value, raised: &[String]) -> String {
  let failure = parse(document, raised).err().unwrap();
  assert_eq!(failure.code, "plan_invalid");
  failure.message
}

#[test]
fn captured_review_items_parse_and_get_heuristic_tiers() {
  let items = parse(&fixture::json("items/review-items.json"), &[]).unwrap();
  let tiers: Vec<usize> = items.iter().map(heuristic).collect();
  assert_eq!(tiers, [3, 2, 2, 1, 0, 1]);
  assert_eq!(items[0].path.as_deref(), Some("src/store/stats_counts.rs"));
  assert_eq!(items[0].severity.as_deref(), Some("medium"));
}

#[test]
fn each_broken_rule_is_named() {
  let item = |id: &str| json!({"id": id, "kind": "thread", "task": "fix"});
  let cases = [
    (json!({"items": []}), "items must be a non-empty array"),
    (json!({"items": {}}), "items must be a non-empty array"),
    (json!({"items": [null]}), "every item needs an id"),
    (json!({"items": [{"id": ""}]}), "every item needs an id"),
    (
      json!({"items": [item("a"), item("a")]}),
      "item ids must be unique",
    ),
    (
      json!({"items": [{"id": "a", "kind": "ci"}]}),
      "every item needs a task",
    ),
    (
      json!({"items": [{"id": "a", "task": "t", "kind": "issue"}]}),
      "item kind must be thread, conversation, review or ci",
    ),
    (
      json!({"round": 0, "items": [item("a")]}),
      "round must be a positive integer",
    ),
    (
      json!({"round": 1.5, "items": [item("a")]}),
      "round must be a positive integer",
    ),
  ];
  for (document, expected) in cases {
    assert_eq!(message(&document, &[]), expected, "{document}");
  }
  let raised = ["b".to_owned()];
  assert_eq!(
    message(&json!({"items": [item("a")]}), &raised),
    "--raise names an item that is not in the items file"
  );
}

#[test]
fn wording_and_severity_pick_the_heuristic_tier() {
  let tier = |severity: Option<&str>, task: &str| {
    let document =
      json!({"items": [{"id": "a", "kind": "thread", "task": task, "severity": severity}]});
    heuristic(&parse(&document, &[]).unwrap()[0])
  };
  assert_eq!(tier(Some("CRITICAL"), "rename x"), 3);
  assert_eq!(tier(None, "Possible data-loss on retry"), 3);
  assert_eq!(tier(Some("high"), "typo"), 2);
  assert_eq!(tier(Some("nit"), "Fix the typos in docs"), 0);
  assert_eq!(tier(Some("medium"), "typo"), 1);
  assert_eq!(tier(None, "Handle the empty list"), 1);
}
