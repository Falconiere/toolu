use serde_json::{Value, json};

use super::insert;

#[test]
fn a_missing_object_is_created_and_a_non_object_is_refused() {
  let mut document = json!({"version": 1, "gates": "no"});
  let Value::Object(map) = &mut document else {
    panic!("object");
  };
  let err = insert(map, "gates.pushReview", "gates.pushReview", json!("off"));
  assert!(err.is_err());
  assert_eq!(map.get("gates").and_then(Value::as_str), Some("no"));

  let mut fresh = json!({"version": 1});
  let Value::Object(map) = &mut fresh else {
    panic!("object");
  };
  insert(map, "gates.pushReview", "gates.pushReview", json!("off")).expect("insert");
  assert_eq!(map["gates"]["pushReview"], json!("off"));
}
