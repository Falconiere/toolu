use std::time::Duration;

use serde_json::json;
use toolu_http_test_support::{Fixture, Reply};

use super::{ClosePolls, GitHubTracker};
use crate::support::{gh, ok};
use crate::tracker::Tracker;

const QUICK: ClosePolls = ClosePolls {
  count: 2,
  wait: Duration::from_millis(1),
};

fn tracker(fixture: &Fixture) -> GitHubTracker {
  GitHubTracker::with("https://github.com/O/R/issues/1", gh(fixture), QUICK).expect("tracker")
}

fn epic(body: &str, state: &str) -> serde_json::Value {
  json!({"title": "Epic", "state": state, "html_url": "https://github.com/O/R/issues/1", "body": body})
}

#[test]
fn epic_reads_title_state_and_caches_the_body_for_the_task_list() {
  let fixture = Fixture::start().expect("fixture");
  ok(&fixture, "/repos/O/R/issues/1", &epic("- [ ] #2", "closed"));
  ok(
    &fixture,
    "/repos/O/R/issues/1/sub_issues?per_page=100",
    &json!([]),
  );
  let child = json!({"title": "Two", "state": "open", "html_url": "u", "body": ""});
  ok(&fixture, "/repos/O/R/issues/2", &child);
  ok(
    &fixture,
    "/repos/O/R/issues/2/dependencies/blocked_by",
    &json!([]),
  );
  ok(
    &fixture,
    "/graphql",
    &json!({"data": {"repository": {"issue": null}}}),
  );
  let tracker = tracker(&fixture);
  let info = tracker.epic().expect("epic");
  assert_eq!(
    (info.reference.as_str(), info.state.as_str()),
    ("O/R#1", "closed")
  );
  let children = tracker.children().expect("children");
  assert_eq!(children.len(), 1);
  assert_eq!(children[0].reference, "O/R#2");
  let epic_reads = fixture
    .requests()
    .expect("requests")
    .iter()
    .filter(|request| request.path == "/repos/O/R/issues/1")
    .count();
  assert_eq!(epic_reads, 1);
  assert_eq!(tracker.state_slug(), "o-r-1");
}

#[test]
fn close_issue_waits_for_the_pull_request_then_closes_by_hand() {
  let fixture = Fixture::start().expect("fixture");
  let state = tempfile::tempdir().expect("state");
  ok(&fixture, "/repos/O/R/issues/2", &json!({"state": "open"}));
  ok(&fixture, "/repos/O/R/issues/2/comments", &json!({"id": 1}));
  ok(
    &fixture,
    "/repos/O/R/issues/2/comments?per_page=100",
    &json!([]),
  );
  let tracker = tracker(&fixture);
  assert_eq!(
    tracker
      .close_issue("O/R#2", "Delivered", state.path())
      .expect("close"),
    "closed-manually"
  );
  let requests = fixture.requests().expect("requests");
  let methods: Vec<&str> = requests
    .iter()
    .map(|request| request.method.as_str())
    .collect();
  assert_eq!(methods, ["GET", "GET", "GET", "POST", "PATCH"]);
  let patch: serde_json::Value = serde_json::from_slice(&requests[4].body).expect("json");
  assert_eq!(
    patch,
    json!({"state": "closed", "state_reason": "completed"})
  );
  assert!(!tracker.is_closed("O/R#2").expect("state"));
}

#[test]
fn close_issue_reports_an_issue_the_pull_request_closed() {
  let fixture = Fixture::start().expect("fixture");
  let state = tempfile::tempdir().expect("state");
  ok(&fixture, "/repos/O/R/issues/3", &json!({"state": "closed"}));
  assert_eq!(
    tracker(&fixture)
      .close_issue("O/R#3", "x", state.path())
      .expect("close"),
    "closed-by-pr"
  );
  assert_eq!(fixture.requests().expect("requests").len(), 1);
}

#[test]
fn tick_epic_patches_only_a_changed_body() {
  let fixture = Fixture::start().expect("fixture");
  ok(
    &fixture,
    "/repos/O/R/issues/1",
    &epic("- [ ] #2 Two\n- [ ] #3", "open"),
  );
  let tracker = tracker(&fixture);
  assert!(tracker.tick_epic("O/R#2", "Two").expect("tick"));
  assert!(!tracker.tick_epic("O/R#9", "Nine").expect("tick"));
  let requests = fixture.requests().expect("requests");
  let patch = requests
    .iter()
    .find(|request| request.method == "PATCH")
    .expect("patch");
  let body: serde_json::Value = serde_json::from_slice(&patch.body).expect("json");
  assert_eq!(body["body"], "- [x] #2 Two\n- [ ] #3");
}

#[test]
fn close_epic_comments_then_closes() {
  let fixture = Fixture::start().expect("fixture");
  let state = tempfile::tempdir().expect("state");
  ok(&fixture, "/repos/O/R/issues/1/comments", &json!({"id": 1}));
  ok(
    &fixture,
    "/repos/O/R/issues/1/comments?per_page=100",
    &json!([]),
  );
  ok(&fixture, "/repos/O/R/issues/1", &json!({"state": "open"}));
  assert_eq!(
    tracker(&fixture)
      .close_epic("Epic complete.", state.path())
      .expect("close"),
    "closed"
  );
  let methods: Vec<String> = fixture
    .requests()
    .expect("requests")
    .iter()
    .map(|r| r.method.clone())
    .collect();
  assert_eq!(methods, ["GET", "GET", "POST", "PATCH"]);
}

#[cfg(test)]
#[path = "tests/github_close_test.rs"]
mod close_tests;

#[test]
fn a_failed_comment_stops_the_close() {
  let fixture = Fixture::start().expect("fixture");
  let state = tempfile::tempdir().expect("state");
  ok(&fixture, "/repos/O/R/issues/1", &json!({"state": "open"}));
  fixture
    .route("/repos/O/R/issues/1/comments", Reply::new(503, "{}"))
    .expect("route");
  fixture
    .route(
      "/repos/O/R/issues/1/comments?per_page=100",
      Reply::new(200, "[]"),
    )
    .expect("list");
  assert!(tracker(&fixture).close_epic("x", state.path()).is_err());
  assert_eq!(fixture.requests().expect("requests").len(), 3);
}
