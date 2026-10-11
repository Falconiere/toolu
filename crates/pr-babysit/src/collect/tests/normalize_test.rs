use serde_json::{Value, json};

use super::{bot, check, comment, pr, review, thread, thread_comment};
use crate::fixture;

fn pages(name: &str) -> Vec<Value> {
  fixture::json(name).as_array().unwrap().clone()
}

#[test]
fn recorded_pages_normalize_to_the_recorded_counts() {
  let threads: Vec<(Value, Option<Value>)> = pages("github/pages-115-threads-p2.json")
    .iter()
    .flat_map(|page| {
      page["data"]["repository"]["pullRequest"]["reviewThreads"]["nodes"]
        .as_array()
        .unwrap()
        .clone()
    })
    .map(|raw| thread(&raw))
    .collect();
  assert_eq!(threads.len(), 17);
  assert!(threads.iter().all(|(_, overflow)| overflow.is_none()));
  let flat = |name: &str, shape: fn(&Value) -> Value| -> Vec<Value> {
    pages(name)
      .iter()
      .flat_map(|page| page.as_array().unwrap().clone())
      .map(|raw| shape(&raw))
      .collect()
  };
  assert_eq!(flat("github/pages-115-comments-p2.json", comment).len(), 3);
  let reviews = flat("github/pages-115-reviews-p2.json", review);
  assert_eq!(reviews.len(), 20);
  assert_eq!(
    reviews[0],
    fixture::json("snapshots/toolu-115.json")["reviews"][0]
  );
  let extra: Vec<Value> = pages("github/pages-165-thread-comments-p2.json")
    .iter()
    .flat_map(|page| {
      page["data"]["node"]["comments"]["nodes"]
        .as_array()
        .unwrap()
        .clone()
    })
    .map(|raw| thread_comment(&raw))
    .collect();
  assert_eq!(extra.len(), 3);
}

#[test]
fn an_overflowing_thread_reports_its_cursor() {
  let raw = json!({"id": "T", "comments": {"pageInfo": {"hasNextPage": true, "endCursor": "c1"}, "nodes": []}});
  let (doc, cursor) = thread(&raw);
  assert_eq!(cursor, Some(json!("c1")));
  assert_eq!(doc["isResolved"], Value::Null);
  assert!(doc.get("commentsHasNextPage").is_none());
}

#[test]
fn checks_and_pull_requests_take_gh_pr_views_shape() {
  let status = json!({"__typename": "StatusContext", "context": "ci/x", "state": "FAILURE",
    "targetUrl": "https://ci", "createdAt": "2026-10-11T00:00:00Z"});
  assert_eq!(
    check(&status),
    json!({"__typename": "StatusContext", "context": "ci/x", "startedAt": "2026-10-11T00:00:00Z",
      "state": "FAILURE", "targetUrl": "https://ci"})
  );
  let run = check(&json!({"__typename": "CheckRun", "name": "rust", "status": "QUEUED"}));
  assert_eq!(run["workflowName"], "");
  assert_eq!(run["conclusion"], Value::Null);
  let doc = pr(
    &json!({"number": 1, "reviewDecision": null, "author": {"login": "me"}}),
    &[run],
  );
  assert_eq!(doc["reviewDecision"], "");
  assert_eq!(doc["author"], "me");
  assert_eq!(doc["statusCheckRollup"].as_array().map(Vec::len), Some(1));
}

#[test]
fn an_absent_bot_comment_differs_from_a_blank_one() {
  let absent = bot(&[]);
  assert_eq!(absent["comment"], Value::Null);
  assert_eq!(absent["verdict"]["state"], "absent");
  let blank =
    bot(&[json!({"id": 9001, "body": "", "author": "github-actions", "authorType": "Bot"})]);
  assert_eq!(blank["comment"]["id"], 9001);
  assert_eq!(blank["verdict"]["state"], "unknown");
  assert_eq!(blank["verdict"]["is_review_comment"], false);
}

#[test]
fn the_latest_review_comment_wins_over_a_later_chat_comment() {
  let review_body = fixture::text("verdicts/pr122-verdict-approved.txt");
  let comments = [
    json!({"id": 1, "body": review_body, "author": "github-actions[bot]"}),
    json!({"id": 2, "body": "thanks!", "author": "claude"}),
    json!({"id": 3, "body": "lgtm", "author": "Falconiere"}),
  ];
  let chosen = bot(&comments);
  assert_eq!(chosen["comment"]["id"], 1);
  assert_eq!(chosen["verdict"]["is_review_comment"], true);
}
