use serde_json::json;
use toolu_http_test_support::{Fixture, Reply};

use super::{SubIssue, details, labels, ref_of, sub_issues};
use crate::refs::parse_ref;
use crate::support::{gh, ok};

fn issue(owner: &str, repo: &str, number: u64, state: &str, body: &str) -> serde_json::Value {
  json!({
    "repository_url": format!("https://api.example.test/repos/{owner}/{repo}"),
    "number": number,
    "title": format!("Issue {number}"),
    "state": state,
    "html_url": format!("https://github.com/{owner}/{repo}/issues/{number}"),
    "body": body,
    "labels": [{"name": "area"}, "plain", {"name": ""}],
  })
}

fn closing(fixture: &Fixture, prs: &serde_json::Value) {
  let body =
    json!({"data": {"repository": {"issue": {"closedByPullRequestsReferences": {"nodes": prs}}}}});
  ok(fixture, "/graphql", &body);
}

#[test]
fn native_sub_issues_are_listed_with_their_repository() {
  let fixture = Fixture::start().expect("fixture");
  let subs = json!([
    issue("o", "r", 2, "open", ""),
    issue("x", "y", 3, "closed", "")
  ]);
  ok(
    &fixture,
    "/repos/o/r/issues/1/sub_issues?per_page=100",
    &subs,
  );
  let found = sub_issues(&gh(&fixture), &parse_ref("o/r#1", None).expect("ref"), "").expect("subs");
  let refs: Vec<&str> = found.iter().map(|sub| sub.reference.as_str()).collect();
  assert_eq!(refs, ["o/r#2", "x/y#3"]);
  assert_eq!(found[1].state, "closed");
}

#[test]
fn without_sub_issues_the_task_list_names_the_children() {
  let fixture = Fixture::start().expect("fixture");
  ok(
    &fixture,
    "/repos/o/r/issues/1/sub_issues?per_page=100",
    &json!([]),
  );
  ok(
    &fixture,
    "/repos/o/r/issues/4",
    &issue("o", "r", 4, "open", ""),
  );
  ok(
    &fixture,
    "/repos/x/y/issues/5",
    &issue("x", "y", 5, "open", ""),
  );
  let body = "intro #9\n- [ ] #4 first\n- [x] x/y#5 second\n- [ ] #4 again\n* not a task #6";
  let found =
    sub_issues(&gh(&fixture), &parse_ref("o/r#1", None).expect("ref"), body).expect("subs");
  let refs: Vec<&str> = found.iter().map(|sub| sub.reference.as_str()).collect();
  assert_eq!(refs, ["o/r#4", "x/y#5"]);
}

#[test]
fn details_join_native_and_text_blockers_labels_and_closing_prs() {
  let fixture = Fixture::start().expect("fixture");
  ok(
    &fixture,
    "/repos/o/r/issues/2/dependencies/blocked_by",
    &json!([issue("o", "r", 7, "closed", "")]),
  );
  let body = "Blocked by #8 and o/r#7.\nDepends on x/y#9 and #2.";
  ok(
    &fixture,
    "/repos/o/r/issues/2",
    &issue("o", "r", 2, "open", body),
  );
  ok(
    &fixture,
    "/repos/o/r/issues/8",
    &issue("o", "r", 8, "open", ""),
  );
  ok(
    &fixture,
    "/repos/x/y/issues/9",
    &issue("x", "y", 9, "closed", ""),
  );
  closing(
    &fixture,
    &json!([{"number": 11, "state": "OPEN", "url": "u", "headRefName": "feat/2-x"}]),
  );
  let sub = SubIssue {
    reference: "o/r#2".into(),
    title: "Two".into(),
    state: "open".into(),
    url: "https://github.com/o/r/issues/2".into(),
  };
  let tracked = details(&gh(&fixture), &sub).expect("details");
  assert_eq!(tracked.deps_source, "native+text");
  assert_eq!(tracked.number, Some(2));
  assert_eq!(tracked.repo, "o/r");
  let blockers: Vec<(&str, &str)> = tracked
    .blockers
    .iter()
    .map(|(k, v)| (k.as_str(), v.as_str()))
    .collect();
  assert_eq!(
    blockers,
    [("o/r#7", "closed"), ("o/r#8", "open"), ("x/y#9", "closed")]
  );
  assert_eq!(tracked.labels, ["area", "plain"]);
  assert_eq!(tracked.prs.len(), 1);
  assert_eq!(tracked.prs[0].head_ref_name, "feat/2-x");
  assert!(tracked.excerpt.starts_with("Blocked by #8"));
}

#[test]
fn an_absent_or_closed_dependency_api_falls_back_to_text_blockers() {
  for (status, body) in [
    (404, r#"{"message":"Not Found"}"#),
    (
      403,
      r#"{"message":"Resource not accessible by integration"}"#,
    ),
  ] {
    let fixture = Fixture::start().expect("fixture");
    fixture
      .route(
        "/repos/o/r/issues/2/dependencies/blocked_by",
        Reply::new(status, body),
      )
      .expect("route");
    ok(
      &fixture,
      "/repos/o/r/issues/2",
      &issue("o", "r", 2, "open", "no deps"),
    );
    closing(&fixture, &json!([]));
    let sub = SubIssue {
      reference: "o/r#2".into(),
      title: "Two".into(),
      state: "closed".into(),
      url: String::new(),
    };
    let tracked = details(&gh(&fixture), &sub).expect("details");
    assert_eq!(tracked.deps_source, "text", "{status}");
    assert_eq!(tracked.state, "closed");
    assert_eq!(tracked.blockers.len(), 0);
  }
}

#[test]
fn ref_of_reads_the_repository_url_and_refuses_a_bad_one() {
  assert_eq!(
    ref_of(&issue("o", "r", 3, "open", "")),
    Ok("o/r#3".to_owned())
  );
  assert!(ref_of(&json!({"repository_url": "", "number": 3})).is_err());
  assert_eq!(labels(&json!({})), Vec::<String>::new());
}

#[test]
fn a_rate_limited_dependency_api_fails_instead_of_dropping_native_blockers() {
  let fixture = Fixture::start().expect("fixture");
  let limited = Reply::new(403, "{\"message\":\"API rate limit exceeded\"}")
    .header("x-ratelimit-remaining", "0")
    .header("x-ratelimit-reset", "4102444800");
  fixture
    .route("/repos/o/r/issues/2/dependencies/blocked_by", limited)
    .expect("route");
  ok(
    &fixture,
    "/repos/o/r/issues/2",
    &issue("o", "r", 2, "open", ""),
  );
  closing(&fixture, &json!([]));
  let sub = SubIssue {
    reference: "o/r#2".into(),
    title: "Two".into(),
    state: "open".into(),
    url: String::new(),
  };
  let err = details(&gh(&fixture), &sub).expect_err("rate limited");
  assert!(err.contains("rate limit"), "{err}");
}
