use serde_json::json;

use super::additive;

#[test]
fn additions_are_additive() {
  assert!(additive(&json!({"a": [1]}), &json!({"a": [1, 2], "b": 3})));
  assert!(additive(
    &json!({"core": [["p"], ["r"]]}),
    &json!({"core": [["p"], ["r", "g"], ["e"]]})
  ));
  assert!(additive(
    &json!({"entries": [{"id": "a"}]}),
    &json!({"entries": [{"id": "z"}, {"id": "a"}]})
  ));
  assert!(additive(&json!(["b", "a"]), &json!(["a", "c", "b"])));
}

#[test]
fn removals_and_changes_are_not() {
  assert!(!additive(&json!({"a": 1}), &json!({})));
  assert!(!additive(&json!({"a": 1}), &json!({"a": 2})));
  assert!(!additive(
    &json!({"core": [["p"], ["r"]]}),
    &json!({"core": [["r"], ["p"]]})
  ));
  assert!(!additive(
    &json!({"core": [["p"], ["r"]]}),
    &json!({"core": [["p"]]})
  ));
  assert!(!additive(
    &json!({"entries": [{"id": "a", "pass": "x"}]}),
    &json!({"entries": [{"id": "a", "pass": "y"}]})
  ));
  assert!(!additive(&json!(["a"]), &json!("a")));
}
