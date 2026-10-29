use serde_json::{Value, json};
use toolu_http_test_support::{Fixture, Reply};

use super::LinearTracker;
use crate::support::{FAST, http_client, proxy_env};
use crate::tracker::Tracker;
use crate::tracker::rest::Http;

fn tracker(fixture: &Fixture, reference: &str, key: &str) -> LinearTracker {
  let env = proxy_env(fixture)
    .with("LINEAR_API_KEY", key)
    .with("LINEAR_API_URL", &fixture.url("/graphql"));
  let http = Http::with_client(http_client(fixture), FAST, "Linear");
  LinearTracker::with(reference, "acme/mono", &env, http).expect("tracker")
}

fn replies(fixture: &Fixture, bodies: &[Value]) {
  let replies = bodies
    .iter()
    .map(|body| Reply::new(200, json!({"data": body}).to_string()))
    .collect();
  fixture.sequence("/graphql", replies).expect("route");
}

fn sent(fixture: &Fixture) -> Vec<Value> {
  fixture
    .requests()
    .expect("requests")
    .iter()
    .map(|request| serde_json::from_slice(&request.body).expect("json"))
    .collect()
}

const CHILD: &str = r#"{"id":"uuid-5","identifier":"ENG-5","title":"Add refunds API","url":"https://linear.app/acme/issue/ENG-5","description":"Repo: acme/payments\nImplement refunds.","state":{"type":"started"},"labels":{"nodes":[{"name":"backend"}]},"inverseRelations":{"nodes":[{"type":"blocks","issue":{"identifier":"ENG-4","state":{"type":"completed"}}}]}}"#;

#[test]
fn personal_keys_go_bare_and_oauth_tokens_as_bearer() {
  let fixture = Fixture::start().expect("fixture");
  let epic = json!({"issue": {"id": "u1", "identifier": "ENG-1", "title": "Epic", "url": "u", "state": {"type": "started"}}});
  replies(&fixture, &[epic.clone(), epic]);
  tracker(&fixture, "linear:ENG-1", "lin_api_personal123")
    .epic()
    .expect("epic");
  tracker(&fixture, "linear:ENG-1", "lin_oauth_token456")
    .epic()
    .expect("epic");
  let auth: Vec<String> = fixture
    .requests()
    .expect("requests")
    .iter()
    .map(|request| {
      request
        .headers
        .get("authorization")
        .cloned()
        .unwrap_or_default()
    })
    .collect();
  assert_eq!(auth, ["lin_api_personal123", "Bearer lin_oauth_token456"]);
}

#[test]
fn a_missing_key_or_a_bad_reference_is_refused_before_any_request() {
  let fixture = Fixture::start().expect("fixture");
  let http = Http::with_client(http_client(&fixture), FAST, "Linear");
  let err = LinearTracker::with("ENG-1", "a/b", &proxy_env(&fixture), http).err();
  assert_eq!(err.as_deref(), Some("LINEAR_API_KEY is not set"));
  let http = Http::with_client(http_client(&fixture), FAST, "Linear");
  let env = proxy_env(&fixture).with("LINEAR_API_KEY", "k");
  let err = LinearTracker::with("12", "a/b", &env, http).err();
  assert_eq!(
    err.as_deref(),
    Some("not a Linear issue identifier or URL: 12")
  );
  assert_eq!(fixture.requests().expect("requests").len(), 0);
}

#[test]
fn epic_and_children_are_normalized_from_graphql() {
  let fixture = Fixture::start().expect("fixture");
  let child: Value = serde_json::from_str(CHILD).expect("child");
  let epic = json!({"issue": {"id": "u1", "identifier": "ENG-1", "title": "Refunds epic", "url": "https://linear.app/acme/issue/ENG-1", "state": {"type": "started"}}});
  let page = json!({"issue": {"children": {"nodes": [child], "pageInfo": {"hasNextPage": false, "endCursor": null}}}});
  replies(&fixture, &[epic, page]);
  let linear = tracker(&fixture, "linear:ENG-1", "lin_api_k");
  let info = linear.epic().expect("epic");
  assert_eq!(
    (info.title.as_str(), info.state.as_str()),
    ("Refunds epic", "open")
  );
  let children = linear.children().expect("children");
  assert_eq!(children[0].reference, "ENG-5");
  assert_eq!(children[0].repo, "acme/payments");
  assert_eq!(
    children[0].blockers.get("ENG-4").map(String::as_str),
    Some("closed")
  );
  assert_eq!(children[0].deps_source, "relations");
  assert_eq!(
    sent(&fixture)[1]["variables"],
    json!({"id": "ENG-1", "after": null})
  );
  assert_eq!(linear.state_slug(), "linear-eng-1");
}

#[test]
fn closing_comments_first_then_moves_to_the_lowest_completed_state() {
  let fixture = Fixture::start().expect("fixture");
  let state = tempfile::tempdir().expect("state");
  let read = json!({"issue": {"id": "uuid-5", "state": {"type": "started"},
    "team": {"states": {"nodes": [{"id": "st-late", "position": 2}, {"id": "st-done", "position": 1}]}}}});
  replies(
    &fixture,
    &[
      read,
      json!({"commentCreate": {"success": true}}),
      json!({"issueUpdate": {"success": true}}),
    ],
  );
  fixture
    .sequence_when(
      "/graphql",
      "comments(first:100",
      vec![Reply::new(
        200,
        json!({"data": {"issue": {"comments": {
          "nodes": [], "pageInfo": {"hasNextPage": false, "endCursor": null}
        }}}})
        .to_string(),
      )],
    )
    .expect("list");
  let outcome =
    tracker(&fixture, "linear:ENG-1", "k").close_issue("ENG-5", "Delivered in PR 7", state.path());
  assert_eq!(outcome.as_deref(), Ok("completed"));
  let bodies = sent(&fixture);
  assert_eq!(
    bodies[2]["variables"],
    json!({"i": "uuid-5", "b": "Delivered in PR 7"})
  );
  assert_eq!(
    bodies[3]["variables"],
    json!({"id": "uuid-5", "s": "st-done"})
  );
}

#[test]
fn a_project_epic_is_left_open_and_graphql_errors_surface() {
  let fixture = Fixture::start().expect("fixture");
  let state = tempfile::tempdir().expect("state");
  fixture
    .route(
      "/graphql",
      Reply::new(
        200,
        r#"{"errors":[{"message":"bad id"},{"message":"again"}]}"#,
      ),
    )
    .expect("route");
  let project = tracker(&fixture, "https://linear.app/acme/project/sync-abc123", "k");
  assert_eq!(project.reference, "project:abc123");
  assert_eq!(
    project.close_epic("done", state.path()).as_deref(),
    Ok("project-left-open")
  );
  assert_eq!(
    project.epic().err().as_deref(),
    Some("Linear: bad id; again")
  );
  assert_eq!(project.state_slug(), "linear-project-abc123");
}

#[test]
fn a_done_issue_is_left_without_a_second_comment() {
  let fixture = Fixture::start().expect("fixture");
  let state = tempfile::tempdir().expect("state");
  let read = json!({"issue": {"id": "uuid-5", "state": {"type": "completed"},
    "team": {"states": {"nodes": [{"id": "st-done", "position": 1}]}}}});
  replies(&fixture, &[read]);
  let outcome =
    tracker(&fixture, "linear:ENG-1", "k").close_issue("ENG-5", "Delivered in PR 7", state.path());
  assert_eq!(outcome.as_deref(), Ok("already-done"));
  assert_eq!(sent(&fixture).len(), 1);
}

#[cfg(test)]
#[path = "tests/linear_close_test.rs"]
mod close_tests;
