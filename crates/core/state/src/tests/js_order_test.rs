use toolu_runtime::json::ordered::Ordered;

use super::{is_array_index, js_order, js_ordered};

#[test]
fn canonical_indices_below_two_to_the_32_minus_one_are_indices() {
  for index in ["0", "10", "4294967294"] {
    assert!(is_array_index(index), "{index}");
  }
  for key in ["4294967295", "01", "-1", "+1", "1.0", "", "b", "/r/a.ts"] {
    assert!(!is_array_index(key), "{key}");
  }
}

#[test]
fn indices_come_first_ascending_and_the_rest_keep_their_order() {
  let keys = ["b", "10", "4294967295", "4294967294", "01", "2", "-1"];
  let ordered: Vec<String> = js_order(keys.iter().map(|key| ((*key).to_owned(), ())).collect())
    .into_iter()
    .map(|(key, ())| key)
    .collect();
  assert_eq!(
    ordered,
    ["2", "10", "4294967294", "b", "4294967295", "01", "-1"]
  );
}

#[test]
fn every_object_at_every_depth_is_reordered_as_json_parse_orders_it() {
  let text = r#"{"b":{"z":1,"3":{"y":[{"q":0,"1":0}],"0":"x"}},"10":[],"a":null}"#;
  let ordered = js_ordered(Ordered::parse(text).unwrap());
  assert_eq!(
    ordered.to_text(false),
    r#"{"10":[],"b":{"3":{"0":"x","y":[{"1":0,"q":0}]},"z":1},"a":null}"#
  );
  let scalar = js_ordered(Ordered::parse("[\"4294967295\"]").unwrap());
  assert_eq!(scalar.to_text(false), "[\"4294967295\"]");
}
