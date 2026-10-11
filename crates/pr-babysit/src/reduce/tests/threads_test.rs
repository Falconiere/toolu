use serde_json::json;

use super::{conversation, reviews, threads};

#[test]
fn bots_flagged_and_authored_threads_are_not_actionable() {
  let author = json!("me");
  let snapshot = json!({"threads": [
    {"id": "t1", "isResolved": false, "isOutdated": false, "comments": [{"author": "dependabot", "authorType": "Bot", "databaseId": 1}]},
    {"id": "t2", "isResolved": false, "isOutdated": false, "comments": [{"author": "rev", "authorType": "User", "databaseId": 2}]},
    {"id": "t3", "isResolved": false, "isOutdated": false, "comments": [{"author": "rev", "authorType": "User", "databaseId": 3}, {"author": "me", "databaseId": 4}]},
    {"id": "t4", "isResolved": true, "comments": [{"author": "rev", "databaseId": 5}]},
  ]});
  let actions = json!({"flagged": {"t2": {}}, "replied": {}});
  let sets = threads(&snapshot, &author, &actions, &|_| false);
  assert_eq!(sets.actionable.len(), 0);
  assert_eq!(sets.flagged, [json!("t2")]);
  let stale: Vec<&str> = sets
    .stale
    .iter()
    .map(|item| item["id"].as_str().unwrap())
    .collect();
  assert_eq!(stale, ["t1", "t3"]);
  assert_eq!(
    sets.unresolved, 2,
    "the bot thread and the replied human thread are audited"
  );
}

#[test]
fn conversation_and_reviews_skip_answered_bot_and_approved_items() {
  let author = json!("me");
  let snapshot = json!({
    "comments": [
      {"id": 1, "author": "rev", "authorType": "User", "createdAt": "2026-01-01T00:00:00Z", "body": "a"},
      {"id": 2, "author": "me", "authorType": "User", "createdAt": "2026-01-02T00:00:00Z", "body": "b"},
      {"id": 3, "author": "rev", "authorType": "User", "createdAt": "2026-01-03T00:00:00Z", "body": "c"},
      {"id": 4, "author": "ci", "authorType": "Bot", "createdAt": "2026-01-04T00:00:00Z", "body": "d"},
      {"id": 5, "author": "rev", "authorType": "User", "createdAt": "2026-01-05T00:00:00Z", "body": "e"},
    ],
    "reviews": [
      {"id": 7, "author": "rev", "authorType": "User", "state": "APPROVED", "body": "ok"},
      {"id": 8, "author": "rev", "authorType": "User", "state": "COMMENTED", "body": ""},
      {"id": 9, "author": "rev", "authorType": "User", "state": "CHANGES_REQUESTED", "body": "fix"},
    ],
  });
  let replied = json!({"conversation:5": {}});
  let (open, fixing) = conversation(&snapshot, &author, Some(&replied), &|item| item["id"] == 3);
  assert_eq!(open.len(), 0);
  assert_eq!(fixing.len(), 1);
  let (open, _) = reviews(&snapshot, &author, Some(&replied), &|_| false);
  assert_eq!(open.len(), 1);
  assert_eq!(open[0]["id"], 9);
  assert!(open[0].get("authorType").is_none());
}
