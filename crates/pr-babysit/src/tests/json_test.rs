use serde_json::{Value, json};

use super::{array, has_key, jq_string, nil, or, round, text};

#[test]
fn a_round_is_a_whole_number_of_at_least_one() {
  for (document, expected) in [
    (json!({}), Some(1)),
    (json!({"round": null}), Some(1)),
    (json!({"round": 2}), Some(2)),
    (json!({"round": 2.0}), Some(2)),
    (json!({"round": 2.5}), None),
    (json!({"round": 0}), None),
    (json!({"round": -1}), None),
    (json!({"round": "2"}), None),
  ] {
    assert_eq!(round(&document), expected, "{document}");
  }
}

#[test]
fn null_false_and_absent_fall_back() {
  let doc = json!({"a": null, "b": false, "c": 0, "d": "x"});
  assert_eq!(nil(doc.get("a")), None);
  assert_eq!(nil(doc.get("b")), None);
  assert_eq!(nil(doc.get("z")), None);
  assert_eq!(nil(doc.get("c")), Some(&json!(0)));
  assert_eq!(or(doc.get("b"), json!("f")), json!("f"));
  assert_eq!(or(doc.get("d"), json!("f")), json!("x"));
}

#[test]
fn arrays_strings_and_keys_read_safely() {
  let doc = json!({"list": [1, 2], "map": {"k": 1}, "s": "v", "n": 7});
  assert_eq!(array(doc.get("list")).len(), 2);
  assert_eq!(array(doc.get("s")).len(), 0);
  assert_eq!(array(None).len(), 0);
  assert_eq!(jq_string(&json!("id")), "id");
  assert_eq!(jq_string(&json!(42)), "42");
  assert_eq!(jq_string(&Value::Null), "null");
  assert_eq!(text(&doc, "s"), "v");
  assert_eq!(text(&doc, "n"), "");
  assert!(has_key(doc.get("map"), "k"));
  assert!(!has_key(doc.get("map"), "x"));
  assert!(!has_key(doc.get("s"), "k"));
}
