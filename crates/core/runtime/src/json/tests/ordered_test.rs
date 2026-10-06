use serde_json::json;

use super::Ordered;

#[test]
fn objects_keep_document_order_through_a_round_trip() {
  let text = r#"{"z":1,"a":{"y":[true,null],"b":"x"},"m":1.50}"#;
  let value = Ordered::parse(text).unwrap();
  assert_eq!(
    value.to_text(false),
    r#"{"z":1,"a":{"y":[true,null],"b":"x"},"m":1.5}"#
  );
  let pretty = "{\n  \"z\": 1,\n  \"a\": {\n    \"y\": [\n      true,\n      null\n    ],\n    \"b\": \"x\"\n  },\n  \"m\": 1.5\n}";
  assert_eq!(value.to_text(true), pretty);
}

#[test]
fn a_repeated_key_keeps_its_first_position_and_last_value() {
  let value = Ordered::parse(r#"{"a":1,"b":2,"a":3}"#).unwrap();
  assert_eq!(value.to_text(false), r#"{"a":3,"b":2}"#);
  assert_eq!(value.get("a"), Some(&Ordered::Number(3.into())));
}

#[test]
fn set_replaces_in_place_or_appends_and_ignores_non_objects() {
  let mut value = Ordered::parse(r#"{"a":1,"b":2}"#).unwrap();
  value.set("a", Ordered::Bool(false));
  value.set("c", Ordered::Array(Vec::new()));
  assert_eq!(value.to_text(false), r#"{"a":false,"b":2,"c":[]}"#);
  let mut list = Ordered::parse("[1]").unwrap();
  list.set("a", Ordered::Null);
  assert_eq!(list.to_text(false), "[1]");
  assert_eq!(list.get("a"), None);
}

#[test]
fn a_serde_value_converts_in_map_order_and_bad_text_is_an_error() {
  let value = Ordered::from(&json!({ "b": [1, -2, 0.5, "s"], "a": {} }));
  assert_eq!(value.to_text(false), r#"{"a":{},"b":[1,-2,0.5,"s"]}"#);
  assert!(Ordered::parse("{").is_err());
  assert!(Ordered::parse("1e400").is_err());
}
