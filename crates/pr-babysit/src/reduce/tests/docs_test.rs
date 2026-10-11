use serde_json::json;

use super::comparison;
use crate::reduce::inputs::bot;

#[test]
fn the_comparison_lists_the_eight_change_fields() {
  let bot = bot(&json!({}), None);
  let fields = comparison(&json!({"mergeable": "MERGEABLE"}), "pass", 2, "abc", &bot);
  let keys: Vec<&str> = fields.iter().map(|(key, _)| *key).collect();
  assert_eq!(
    keys,
    [
      "ciStatus",
      "reviewDecision",
      "mergeable",
      "unresolvedThreads",
      "headSha",
      "botVerdict",
      "botState",
      "botFindingKeys"
    ]
  );
  assert_eq!(fields[1].1, serde_json::Value::Null);
  assert_eq!(fields[2].1, "MERGEABLE");
  assert_eq!(fields[3].1, 2);
  assert_eq!(fields[6].1, "absent");
}
