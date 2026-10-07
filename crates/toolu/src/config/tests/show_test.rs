use serde_json::json;

use super::render_value;

#[test]
fn a_string_is_raw_and_an_object_is_pretty() {
  assert_eq!(render_value(&json!("off")), "off");
  let pretty = render_value(&json!({"version": 1}));
  assert!(pretty.contains('\n'), "{pretty}");
  assert!(pretty.contains("\"version\""), "{pretty}");
}
