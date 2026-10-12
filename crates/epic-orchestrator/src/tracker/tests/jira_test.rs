use serde_json::{Value, json};
use toolu_http_test_support::{Fixture, Reply};

use super::{JiraTracker, adf_text, jira_blockers};
use crate::support::{FAST, http_client, jira_env};
use crate::tracker::Tracker;
use crate::tracker::jira_client::JiraClient;
use crate::tracker::rest::Http;

fn tracker(fixture: &Fixture) -> JiraTracker {
  let http = Http::with_client(http_client(fixture), FAST, "epic jira");
  let client = JiraClient::with_http(&jira_env(fixture, "3"), http).expect("client");
  JiraTracker::with("PAY-7", "acme/payments", client).expect("tracker")
}

fn issue(key: &str, done: bool) -> Value {
  json!({"key": key, "fields": {
    "summary": format!("{key} work"),
    "status": {"statusCategory": {"key": if done { "done" } else { "new" }}},
    "description": {"type": "doc", "content": [
      {"type": "paragraph", "content": [{"type": "text", "text": "Repo: acme/refunds"}]}
    ]},
    "labels": ["backend"],
    "issuelinks": [{"type": {"inward": "is blocked by", "outward": "blocks"},
      "inwardIssue": {"key": "PAY-11", "fields": {"status": {"statusCategory": {"key": "new"}}}}}],
  }})
}

fn route(fixture: &Fixture, path: &str, body: &Value) {
  fixture
    .route(path, Reply::new(200, body.to_string()))
    .expect("route");
}

#[test]
fn the_epic_and_its_children_are_normalized() {
  let fixture = Fixture::start().expect("fixture");
  route(
    &fixture,
    "/rest/api/3/issue/PAY-7?fields=summary,status",
    &issue("PAY-7", true),
  );
  route(
    &fixture,
    "/rest/api/3/search/jql",
    &json!({"issues": [issue("PAY-12", false)]}),
  );
  let jira = tracker(&fixture);
  let epic = jira.epic().expect("epic");
  assert_eq!(
    (epic.title.as_str(), epic.state.as_str()),
    ("PAY-7 work", "closed")
  );
  assert_eq!(
    epic.url,
    format!("{}/browse/PAY-7", fixture.url("").trim_end_matches('/'))
  );
  let children = jira.children().expect("children");
  assert_eq!(children[0].reference, "PAY-12");
  assert_eq!(children[0].repo, "acme/refunds");
  assert_eq!(
    children[0].blockers.get("PAY-11").map(String::as_str),
    Some("open")
  );
  assert_eq!(children[0].deps_source, "links");
  assert_eq!(jira.state_slug(), "jira-pay-7");
}

#[test]
fn an_empty_parent_search_falls_back_to_epic_link_and_tolerates_its_400() {
  let fixture = Fixture::start().expect("fixture");
  fixture
    .sequence(
      "/rest/api/3/search/jql",
      vec![
        Reply::new(200, r#"{"issues":[]}"#),
        Reply::new(
          400,
          r#"{"errorMessages":["Field 'Epic Link' does not exist"]}"#,
        ),
      ],
    )
    .expect("route");
  assert_eq!(tracker(&fixture).children().expect("children"), Vec::new());
  let bodies: Vec<Value> = fixture
    .requests()
    .expect("requests")
    .iter()
    .map(|request| serde_json::from_slice(&request.body).expect("json"))
    .collect();
  assert_eq!(bodies[1]["jql"], "\"Epic Link\" = PAY-7 ORDER BY key");
}

#[test]
fn close_issue_skips_done_items_and_transitions_open_ones() {
  let fixture = Fixture::start().expect("fixture");
  route(
    &fixture,
    "/rest/api/3/issue/PAY-1?fields=status",
    &issue("PAY-1", true),
  );
  route(
    &fixture,
    "/rest/api/3/issue/PAY-2?fields=status",
    &issue("PAY-2", false),
  );
  route(&fixture, "/rest/api/3/issue/PAY-2/comment", &json!({}));
  let done = json!({"transitions": [{"id": "31", "name": "Done", "to": {"statusCategory": {"key": "done"}}}]});
  route(&fixture, "/rest/api/3/issue/PAY-2/transitions", &done);
  let jira = tracker(&fixture);
  assert_eq!(
    jira.close_issue("PAY-1", "x").expect("close"),
    "already-done"
  );
  assert_eq!(
    jira.close_issue("PAY-2", "Delivered").expect("close"),
    "transitioned-done"
  );
  let last = fixture
    .requests()
    .expect("requests")
    .pop()
    .expect("request");
  assert_eq!(
    serde_json::from_slice::<Value>(&last.body).expect("json"),
    json!({"transition": {"id": "31"}})
  );
}

#[test]
fn close_epic_comments_in_adf_and_reports_a_missing_done_transition() {
  let fixture = Fixture::start().expect("fixture");
  route(&fixture, "/rest/api/3/issue/PAY-7/comment", &json!({}));
  route(
    &fixture,
    "/rest/api/3/issue/PAY-7/transitions",
    &json!({"transitions": []}),
  );
  assert_eq!(
    tracker(&fixture).close_epic("Shipped").expect("close"),
    "no-done-transition"
  );
  let first = fixture.requests().expect("requests").remove(0);
  let body: Value = serde_json::from_slice(&first.body).expect("json");
  assert_eq!(body["body"]["content"][0]["content"][0]["text"], "Shipped");
  assert!(!tracker(&fixture).tick_epic("PAY-1", "t").expect("tick"));
}

#[test]
fn blockers_come_from_both_link_directions_and_blocking_wordings() {
  let done = json!({"statusCategory": {"key": "done"}});
  let todo = json!({"statusCategory": {"key": "new"}});
  let links = json!([
    {"type": {"inward": "is blocked by", "outward": "blocks"}, "inwardIssue": {"key": "PAY-1", "fields": {"status": done}}},
    {"type": {"inward": "is blocked by", "outward": "blocks"}, "outwardIssue": {"key": "PAY-9", "fields": {"status": todo}}},
    {"type": {"inward": "is depended on by", "outward": "depends on"}, "outwardIssue": {"key": "PAY-3", "fields": {"status": todo}}},
    {"type": {"inward": "relates to", "outward": "relates to"}, "outwardIssue": {"key": "PAY-4"}},
    {"type": {"inward": "x", "outward": "y"}}
  ]);
  let found: Vec<(String, String)> = jira_blockers(Some(&links)).into_iter().collect();
  assert_eq!(
    found,
    [
      ("PAY-1".into(), "closed".into()),
      ("PAY-3".into(), "open".into())
    ]
  );
  assert_eq!(jira_blockers(None), std::collections::BTreeMap::new());
}

#[test]
fn adf_descriptions_become_text() {
  let doc = json!({"type": "doc", "content": [
    {"type": "paragraph", "content": [{"type": "text", "text": "Repo: acme/api"}]},
    {"type": "paragraph", "content": [{"type": "text", "text": "Second line"}]}
  ]});
  assert!(adf_text(&doc).contains("Repo: acme/api\n"));
  assert_eq!(adf_text(&json!("plain v2 text")), "plain v2 text");
  assert_eq!(adf_text(&json!(4)), "");
  let bad = JiraTracker::with("12", "a/b", jira_client_for_refusal());
  assert_eq!(
    bad.err().as_deref(),
    Some("not a Jira issue key or browse URL: 12")
  );
}

fn jira_client_for_refusal() -> JiraClient {
  let fixture = Fixture::start().expect("fixture");
  let http = Http::with_client(http_client(&fixture), FAST, "epic jira");
  JiraClient::with_http(&jira_env(&fixture, "2"), http).expect("client")
}
