use serde_json::json;
use toolu_http_test_support::{Fixture, Reply};

use super::{details, sub_issues};
use crate::refs::parse_ref;
use crate::support::{gh, ok};

fn native_children(fixture: &Fixture) -> Vec<super::SubIssue> {
  sub_issues(
    &gh(fixture),
    &parse_ref("Falconiere/toolu#737", None).expect("ref"),
    "",
  )
  .expect("native children")
}

fn closing(fixture: &Fixture) {
  let body =
    json!({"data": {"repository": {"issue": {"closedByPullRequestsReferences": {"nodes": []}}}}});
  ok(fixture, "/graphql", &body);
}

fn child_routes(fixture: &Fixture, listed: &serde_json::Value, detail: &serde_json::Value) {
  ok(
    fixture,
    "/repos/Falconiere/toolu/issues/737/sub_issues?per_page=100",
    listed,
  );
  ok(
    fixture,
    "/repos/Falconiere/toolu/issues/760/dependencies/blocked_by",
    &json!([]),
  );
  ok(fixture, "/repos/Falconiere/toolu/issues/760", detail);
  closing(fixture);
}

#[test]
fn captured_native_child_reuses_its_body_and_labels() {
  let fixture = Fixture::start().expect("fixture");
  let captured: serde_json::Value = serde_json::from_str(include_str!(
    "../../../../../fixtures/epic/native-sub-issue-760.json"
  ))
  .expect("captured native response");
  child_routes(&fixture, &captured, &captured[0]);
  let subs = native_children(&fixture);
  let tracked = details(&gh(&fixture), &subs[0]).expect("details");
  assert_eq!(tracked.reference, "Falconiere/toolu#760");
  assert_eq!(tracked.labels, Vec::<String>::new());
  assert!(tracked.excerpt.starts_with("Parent: #737"));
  assert_eq!(tracked.deps_source, "native+text");
  assert!(tracked.blockers.is_empty());
  assert_eq!(tracked.prs.len(), 0);
  let requests = fixture.requests().expect("requests");
  assert_eq!(
    requests
      .iter()
      .filter(|request| request.path == "/repos/Falconiere/toolu/issues/760")
      .count(),
    0
  );
  assert_eq!(requests.len(), 3);
}

#[test]
fn missing_native_details_fetch_the_issue_once() {
  let captured: serde_json::Value = serde_json::from_str(include_str!(
    "../../../../../fixtures/epic/native-sub-issue-760.json"
  ))
  .expect("captured native response");
  for (field, null) in [
    ("body", false),
    ("body", true),
    ("labels", false),
    ("labels", true),
  ] {
    let fixture = Fixture::start().expect("fixture");
    let mut listed = captured.clone();
    if null {
      listed[0][field] = serde_json::Value::Null;
    } else {
      listed[0].as_object_mut().expect("item").remove(field);
    }
    child_routes(&fixture, &listed, &captured[0]);
    let subs = native_children(&fixture);
    let tracked = details(&gh(&fixture), &subs[0]).expect("details");
    assert!(
      tracked.excerpt.starts_with("Parent: #737"),
      "{field} null={null}"
    );
    assert!(tracked.labels.is_empty(), "{field} null={null}");
    assert_eq!(
      fixture
        .requests()
        .expect("requests")
        .iter()
        .filter(|request| request.path == "/repos/Falconiere/toolu/issues/760")
        .count(),
      1,
      "{field} null={null}"
    );
  }
}

#[test]
fn native_sub_issues_keep_page_order() {
  let fixture = Fixture::start().expect("fixture");
  let first = include_str!("../../../../../fixtures/epic/native-sub-issue-760.json");
  let second = include_str!("../../../../../fixtures/epic/native-sub-issue-761.json");
  let next = fixture.url("/repos/Falconiere/toolu/issues/737/sub_issues?page=2");
  fixture
    .route(
      "/repos/Falconiere/toolu/issues/737/sub_issues?per_page=100",
      Reply::new(200, first).header("Link", &format!("<{next}>; rel=\"next\"")),
    )
    .expect("first page");
  fixture
    .route(
      "/repos/Falconiere/toolu/issues/737/sub_issues?page=2",
      Reply::new(200, second),
    )
    .expect("second page");
  let subs = native_children(&fixture);
  let refs: Vec<&str> = subs.iter().map(|sub| sub.reference.as_str()).collect();
  assert_eq!(refs, ["Falconiere/toolu#760", "Falconiere/toolu#761"]);
  assert_eq!(fixture.requests().expect("requests").len(), 2);
}
