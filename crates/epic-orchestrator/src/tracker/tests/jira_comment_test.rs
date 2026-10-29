use serde_json::json;

use super::comment_matches;

#[test]
fn matches_only_the_text_shape_the_tracker_writes() {
  let delivered = json!({"body": {"type": "doc", "version": 1, "content": [
    {"type": "paragraph", "content": [
      {"type": "text", "text": "Delivered", "marks": []}
    ]}
  ]}});
  let mentioned = json!({"body": {"type": "doc", "content": [
    {"type": "paragraph", "content": [{"type": "mention", "attrs": {"id": "u"}}]}
  ]}});
  assert_eq!(comment_matches(&delivered, "Delivered"), Ok(true));
  assert_eq!(comment_matches(&mentioned, "Delivered"), Ok(false));
  assert_eq!(
    comment_matches(&json!({"body": {"type": "doc"}}), "Delivered"),
    Err("invalid Jira comments response".to_owned())
  );
}
