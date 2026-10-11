use serde_json::{Value, json};
use toolu_http_test_support::{Fixture, Reply};

use super::{Request, collect};
use crate::fixture;
use crate::recorded::{self, ROOT, replies};

const SHA: &str = "c44875dfd8e0bee26d9516cd32af03a2a3c4be44";

fn request(now: &str) -> Request<'_> {
  Request {
    repo: "Falconiere/toolu",
    number: 115,
    page_size: 2,
    now,
  }
}

fn graphql_bodies(fixture: &Fixture) -> Vec<Value> {
  fixture
    .requests()
    .unwrap()
    .into_iter()
    .filter(|request| request.path == "/graphql")
    .map(|request| serde_json::from_slice(&request.body).unwrap())
    .collect()
}

#[test]
fn recorded_pages_reproduce_the_recorded_snapshot_and_the_collector_golden() {
  let fixture = Fixture::start().unwrap();
  recorded::serve(&fixture, &[SHA, SHA], replies(&recorded::round()));
  let captured = recorded::snapshot();
  let now = captured["collectedAt"].as_str().unwrap();
  let collected = collect(&recorded::client(&fixture), &request(now)).unwrap();
  assert_eq!(collected.snapshot, captured);
  let mut golden = collected.snapshot.clone();
  golden.as_object_mut().unwrap().remove("collectedAt");
  assert_eq!(golden, fixture::json("collect-pr-115-p2.golden.json"));
  assert_eq!(
    collected.graphql.points, 10,
    "the PR query and nine thread pages"
  );
  assert_eq!(collected.graphql.remaining, Some(4990));
  assert_eq!(collected.graphql.reset_at, Some(1_760_000_000));
  let requests = fixture.requests().unwrap();
  assert_eq!(
    requests.len(),
    2 + 10 + 2 + 10,
    "heads, GraphQL, comment and review pages"
  );
  let bodies = graphql_bodies(&fixture);
  assert_eq!(bodies[1]["variables"]["endCursor"], Value::Null);
  assert_eq!(
    bodies[2]["variables"]["endCursor"],
    "Y3Vyc29yOnYyOpK0MjAyNi0wNi0xOVQxOToxOTo1MFrOiukZlg=="
  );
}

#[test]
fn a_head_that_moves_once_recollects_and_twice_fails() {
  let fixture = Fixture::start().unwrap();
  let mut pages = recorded::round();
  pages.extend(recorded::round());
  recorded::serve(&fixture, &["old", "new", "new", "new"], replies(&pages));
  let collected = collect(&recorded::client(&fixture), &request("now")).unwrap();
  assert_eq!(
    collected.snapshot["head"],
    json!({"sha": "new", "verifiedAfterFanout": true, "recollected": true})
  );
  assert_eq!(collected.graphql.points, 20);

  let fixture = Fixture::start().unwrap();
  recorded::serve(&fixture, &["a", "b", "c", "d"], replies(&pages));
  let failure = collect(&recorded::client(&fixture), &request("now"))
    .err()
    .unwrap();
  assert_eq!(failure.code, "head_moved");
  assert_eq!(failure.extra["source"], "head");
  let heads = fixture
    .requests()
    .unwrap()
    .iter()
    .filter(|request| request.path == format!("{ROOT}/pulls/115"))
    .count();
  assert_eq!(heads, 4);
}

#[test]
fn overflowing_thread_comments_are_read_through_node_id_and_appended_in_order() {
  let fixture = Fixture::start().unwrap();
  let mut pages = recorded::round();
  let first = &mut pages[1]["data"]["repository"]["pullRequest"]["reviewThreads"]["nodes"][0];
  first["comments"]["pageInfo"] = json!({"hasNextPage": true, "endCursor": "cursor-1"});
  let id = first["id"].clone();
  pages.extend(
    fixture::json("github/pages-165-thread-comments-p2.json")
      .as_array()
      .unwrap()
      .clone(),
  );
  recorded::serve(&fixture, &[SHA, SHA], replies(&pages));
  let collected = collect(&recorded::client(&fixture), &request("now")).unwrap();
  let comments = collected.snapshot["threads"][0]["comments"]
    .as_array()
    .unwrap();
  assert_eq!(comments.len(), 2 + 3);
  let authors: Vec<&str> = comments[2..]
    .iter()
    .map(|c| c["author"].as_str().unwrap())
    .collect();
  assert_eq!(authors, ["github-actions", "Falconiere", "github-actions"]);
  assert_eq!(collected.snapshot["pages"]["threadComments"], 2);
  assert!(
    collected.snapshot["threads"][0]
      .get("commentsHasNextPage")
      .is_none()
  );
  let bodies = graphql_bodies(&fixture);
  assert_eq!(
    bodies[10]["variables"],
    json!({"id": id, "pageSize": 2, "endCursor": "cursor-1"})
  );
  assert_eq!(
    bodies[11]["variables"]["endCursor"],
    "Y3Vyc29yOnYyOpK0MjAyNi0wOC0zMVQxOTozODo1N1rO6FLrzQ=="
  );
}

#[test]
fn graphql_errors_and_missing_pull_requests_are_api_errors_with_no_snapshot() {
  let fixture = Fixture::start().unwrap();
  let body = fixture::text("github/graphql-notfound.out");
  recorded::serve(&fixture, &[SHA, SHA], vec![Reply::new(200, body)]);
  let failure = collect(&recorded::client(&fixture), &request("now"))
    .err()
    .unwrap();
  assert_eq!(
    (failure.code, failure.extra["class"].as_str()),
    ("api_error", Some("graphql"))
  );
  assert_eq!(failure.extra["source"], "pr");
  assert!(
    failure
      .message
      .contains("Could not resolve to a PullRequest")
  );

  let fixture = Fixture::start().unwrap();
  let empty = json!({"data": {"repository": {"pullRequest": null}}}).to_string();
  recorded::serve(&fixture, &[SHA, SHA], vec![Reply::new(200, empty)]);
  let failure = collect(&recorded::client(&fixture), &request("now"))
    .err()
    .unwrap();
  assert_eq!(failure.code, "api_error");
  assert!(failure.message.contains("no such pull request"));
}

#[test]
fn a_missing_pull_request_head_is_a_permanent_api_error() {
  let fixture = Fixture::start().unwrap();
  let message = json!({"message": "Not Found"}).to_string();
  fixture
    .route(&format!("{ROOT}/pulls/115"), Reply::new(404, message))
    .unwrap();
  let failure = collect(&recorded::client(&fixture), &request("now"))
    .err()
    .unwrap();
  assert_eq!(failure.code, "api_error");
  assert_eq!(failure.extra["class"], "permanent");
  assert_eq!(failure.extra["source"], "head");
  assert_eq!(fixture.requests().unwrap().len(), 1, "nothing else is read");
}
