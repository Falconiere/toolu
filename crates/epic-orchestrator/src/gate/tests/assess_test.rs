use serde_json::{Map, Value, json};
use toolu_http_test_support::Fixture;

use super::{Pr, assess, check_buckets, verdict};
use crate::gate_rig::{PrState, serve_pr};
use crate::support::gh;

const ROLLUP: &str = include_str!("../../../../../fixtures/epic/pr290-rollup.json");

fn pr() -> Pr {
  Pr {
    owner: "o".into(),
    repo: "r".into(),
    number: 7,
  }
}

fn names(checks: &Map<String, Value>, bucket: &str) -> Vec<String> {
  serde_json::from_value(checks[bucket].clone()).expect("names")
}

#[test]
fn a_green_rollup_is_all_pass_and_failed_or_running_runs_are_not() {
  let mut rollup: Vec<Value> = serde_json::from_str(ROLLUP).expect("rollup");
  let buckets = check_buckets(&rollup);
  assert_eq!(names(&buckets, "pass").len(), rollup.len());
  assert!(names(&buckets, "fail").is_empty() && names(&buckets, "pending").is_empty());
  rollup[0]["conclusion"] = json!("FAILURE");
  rollup[1]["status"] = json!("IN_PROGRESS");
  rollup[1]["conclusion"] = json!("");
  let buckets = check_buckets(&rollup);
  assert_eq!(
    names(&buckets, "fail"),
    [rollup[0]["name"].as_str().unwrap_or("")]
  );
  assert_eq!(
    names(&buckets, "pending"),
    [rollup[1]["name"].as_str().unwrap_or("")]
  );
}

#[test]
fn status_contexts_bucket_by_state() {
  let contexts = json!([
    {"__typename": "StatusContext", "context": "ci/a", "state": "SUCCESS"},
    {"__typename": "StatusContext", "context": "ci/b", "state": "PENDING"},
    {"__typename": "StatusContext", "context": "ci/c", "state": "ERROR"},
  ]);
  let buckets = check_buckets(contexts.as_array().expect("array"));
  assert_eq!(
    Value::Object(buckets),
    json!({"pass": ["ci/a"], "pending": ["ci/b"], "fail": ["ci/c"]})
  );
}

#[test]
fn verdicts_follow_conflicts_failures_pending_and_green() {
  let case = |facts: Value| {
    let mut reasons = Vec::new();
    let found = verdict(facts.as_object().expect("facts"), &mut reasons);
    (found, reasons)
  };
  let green = json!({"pass": ["t"], "fail": [], "pending": []});
  assert_eq!(
    case(json!({"mergeable": "CONFLICTING", "checks": green})),
    ("rebase", vec!["conflicting with base".to_owned()])
  );
  assert_eq!(
    case(json!({"behind_by": 3, "checks": green})).1,
    ["behind base by 3"]
  );
  let failing = json!({"pass": [], "fail": ["lint"], "pending": []});
  assert_eq!(
    case(json!({"checks": failing, "unresolved_threads": 2, "draft": true})),
    (
      "fix",
      vec![
        "failing: lint".to_owned(),
        "2 unresolved review threads".to_owned(),
        "draft".to_owned()
      ]
    )
  );
  assert_eq!(
    case(json!({"checks": {"pass": [], "fail": [], "pending": ["ci"]}})),
    ("wait", vec!["pending: ci".to_owned()])
  );
  assert_eq!(
    case(json!({"mergeable": "UNKNOWN", "checks": green})).1,
    ["pending: mergeability not computed"]
  );
  assert_eq!(
    case(json!({"mergeable": "MERGEABLE", "checks": green})),
    ("merge", Vec::new())
  );
}

fn with(change: impl FnOnce(&mut PrState)) -> PrState {
  let mut state = PrState::default();
  change(&mut state);
  state
}

fn failing() -> Value {
  json!([{"__typename": "CheckRun", "name": "t", "status": "COMPLETED", "conclusion": "FAILURE"}])
}

#[test]
fn the_fixture_pr_states_give_every_verdict() {
  let cases = [
    (with(|_| {}), "merge"),
    (with(|state| state.behind = 2), "rebase"),
    (with(|state| state.mergeable = "CONFLICTING"), "rebase"),
    (with(|state| state.rollup = failing()), "fix"),
    (with(|state| state.draft = true), "fix"),
    (with(|state| state.rollup = Value::Null), "wait"),
    (with(|state| state.state = "MERGED"), "merged"),
    (with(|state| state.state = "CLOSED"), "closed"),
  ];
  for (state, expected) in cases {
    let fixture = Fixture::start().expect("fixture");
    serve_pr(&fixture, &state);
    let (result, facts) = assess(&gh(&fixture), &pr()).expect("assess");
    assert_eq!(result["verdict"], expected);
    assert_eq!(
      (facts.id.as_str(), result["head"].as_str()),
      ("PR_7", Some("abc123"))
    );
  }
}

#[test]
fn workflows_without_reported_checks_wait() {
  let fixture = Fixture::start().expect("fixture");
  serve_pr(&fixture, &with(|state| state.rollup = Value::Null));
  let (result, _) = assess(&gh(&fixture), &pr()).expect("assess");
  assert_eq!(
    result["checks"]["pending"],
    json!(["(no checks reported yet)"])
  );
  assert_eq!(
    result["reasons"],
    json!(["pending: (no checks reported yet)"])
  );
}

/// A follow-up reply holding one connection page at `at` with `info` as its
/// `pageInfo`.
fn page(at: &str, nodes: &Value, info: &Value) -> toolu_http_test_support::Reply {
  let mut body = json!({"nodes": nodes, "pageInfo": info});
  for key in at.trim_start_matches('/').split('/').rev() {
    body = match key.parse::<usize>() {
      Ok(_) => json!([body]),
      Err(_) => json!({ key: body }),
    };
  }
  toolu_http_test_support::Reply::new(200, json!({ "data": body }).to_string())
}

/// The first PR reply: one green check with more to come (cursor `k1`), and
/// threads with more to come (cursor `t1`) when `threads_next`.
fn first_page(threads_next: bool) -> toolu_http_test_support::Reply {
  let check =
    json!({"__typename": "CheckRun", "name": "a", "status": "COMPLETED", "conclusion": "SUCCESS"});
  let node = json!({"id": "PR_7", "state": "OPEN", "isDraft": false, "mergeable": "MERGEABLE",
    "mergeStateStatus": "CLEAN", "headRefOid": "abc123", "headRefName": "feat/7-x",
    "baseRefName": "main", "url": "", "autoMergeRequest": null,
    "reviewThreads": {"nodes": [], "pageInfo": {"hasNextPage": threads_next, "endCursor": "t1"}},
    "commits": {"nodes": [{"commit": {"statusCheckRollup": {"contexts": {
      "nodes": [check], "pageInfo": {"hasNextPage": true, "endCursor": "k1"}}}}}]}});
  let body = json!({"data": {"repository": {"pullRequest": node}}});
  toolu_http_test_support::Reply::new(200, body.to_string())
}

const CONTEXTS: &str = "/repository/pullRequest/commits/nodes/0/commit/statusCheckRollup/contexts";

/// Assess with `replies` as the GraphQL answers, in order; the result and the
/// cursor each follow-up query sent.
fn paged(replies: Vec<toolu_http_test_support::Reply>) -> (Result<Value, String>, Vec<Value>) {
  let fixture = Fixture::start().expect("fixture");
  serve_pr(&fixture, &PrState::default());
  fixture.sequence("/graphql", replies).expect("pages");
  let result = assess(&gh(&fixture), &pr()).map(|(result, _)| Value::Object(result));
  let cursors = fixture
    .requests()
    .expect("requests")
    .iter()
    .filter(|request| request.path == "/graphql")
    .filter_map(|request| serde_json::from_slice::<Value>(&request.body).ok())
    .filter_map(|body| body["variables"].get("c").cloned())
    .collect();
  (result, cursors)
}

#[test]
fn checks_and_threads_past_the_first_page_count_toward_the_verdict() {
  let threads = "/repository/pullRequest/reviewThreads";
  let open = json!([{"isResolved": false, "isOutdated": false}]);
  let last = json!({"hasNextPage": false, "endCursor": null});
  let more = json!({"hasNextPage": true, "endCursor": "k2"});
  let cases = [
    (
      vec![first_page(false), page(CONTEXTS, &failing(), &last)],
      json!(["k1"]),
      "failing: t",
    ),
    (
      vec![
        first_page(true),
        page(threads, &open, &last),
        page(CONTEXTS, &json!([]), &last),
      ],
      json!(["t1", "k1"]),
      "1 unresolved review threads",
    ),
    (
      vec![
        first_page(false),
        page(CONTEXTS, &json!([]), &more),
        page(CONTEXTS, &failing(), &last),
      ],
      json!(["k1", "k2"]),
      "failing: t",
    ),
  ];
  for (replies, sent, reason) in cases {
    let (result, cursors) = paged(replies);
    let result = result.expect("assess");
    assert_eq!(result["verdict"], "fix", "{result:?}");
    assert_eq!(result["reasons"], json!([reason]));
    assert_eq!(Value::Array(cursors), sent);
  }
}

#[test]
fn an_endless_or_cursorless_connection_fails_instead_of_judging_part_of_it() {
  let more = json!({"hasNextPage": true, "endCursor": "k2"});
  let (endless, cursors) = paged(vec![first_page(false), page(CONTEXTS, &json!([]), &more)]);
  assert_eq!(
    endless.err().as_deref(),
    Some(format!("o/r#7: more than 50 pages at {CONTEXTS}").as_str())
  );
  assert_eq!(cursors.len(), 50);
  let blind = json!({"hasNextPage": true, "endCursor": null});
  let (cursorless, _) = paged(vec![first_page(false), page(CONTEXTS, &json!([]), &blind)]);
  assert_eq!(
    cursorless.err().as_deref(),
    Some("o/r#7: a next page without a cursor")
  );
}
