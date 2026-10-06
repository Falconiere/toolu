use serde_json::json;

use super::{flag_set, list, text};

#[test]
fn present_values_are_read_and_absent_or_mistyped_ones_default() {
  let node =
    json!({"name": "epic", "commands": [{"name": "planned"}], "hidden": true, "global": "yes"});
  assert_eq!(text(&node, "name"), "epic");
  assert_eq!(list(&node, "commands").len(), 1);
  assert!(flag_set(&node, "hidden"));
  assert_eq!(text(&node, "about"), "");
  assert_eq!(list(&node, "name"), &[] as &[serde_json::Value]);
  assert!(!flag_set(&node, "global"));
  assert!(!flag_set(&node, "placeholder"));
}
