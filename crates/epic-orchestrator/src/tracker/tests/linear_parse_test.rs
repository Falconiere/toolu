use serde_json::json;

use super::{child, closed, linear_auth_header, linear_blockers, lowest_completed};

#[test]
fn blockers_come_from_inverse_blocks_relations_only() {
  let issue = json!({"inverseRelations": {"nodes": [
    {"type": "blocks", "issue": {"identifier": "ENG-1", "state": {"type": "completed"}}},
    {"type": "blocks", "issue": {"identifier": "ENG-2", "state": {"type": "unstarted"}}},
    {"type": "related", "issue": {"identifier": "ENG-3", "state": {"type": "unstarted"}}}
  ]}});
  let found: Vec<(String, String)> = linear_blockers(&issue).into_iter().collect();
  assert_eq!(
    found,
    [
      ("ENG-1".into(), "closed".into()),
      ("ENG-2".into(), "open".into())
    ]
  );
}

#[test]
fn auth_headers_follow_the_key_type() {
  assert_eq!(linear_auth_header("Bearer abc"), "Bearer abc");
  assert_eq!(linear_auth_header("bearer\tabc"), "bearer\tabc");
  assert_eq!(linear_auth_header("  lin_api_x  "), "lin_api_x");
  assert_eq!(linear_auth_header("lin_oauth_t"), "Bearer lin_oauth_t");
}

#[test]
fn a_child_falls_back_to_the_default_repository_and_open_state() {
  let node =
    json!({"identifier": "ENG-6", "title": "t", "url": "u", "state": {"type": "canceled"}});
  let item = child(&node, "acme/mono");
  assert_eq!(
    (item.repo.as_str(), item.state.as_str()),
    ("acme/mono", "closed")
  );
  assert_eq!(item.labels, Vec::<String>::new());
  assert!(!closed(None));
}

#[test]
fn the_lowest_completed_state_wins_and_none_is_none() {
  let issue = json!({"team": {"states": {"nodes": [{"id": "b", "position": 2}, {"id": "a", "position": 1}]}}});
  assert_eq!(lowest_completed(&issue).as_deref(), Some("a"));
  assert_eq!(lowest_completed(&json!({})), None);
}
