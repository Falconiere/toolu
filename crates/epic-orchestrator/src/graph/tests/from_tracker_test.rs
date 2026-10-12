use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use toolu_http_test_support::Fixture;

use super::{BuildInput, build};
use crate::graph::Graph;
use crate::support::{FAST, gh, http_client, jira_env, ok, repo_with_origin};
use crate::tracker::Tracker;

fn sub(number: u64, state: &str) -> Value {
  json!({
    "repository_url": "https://api.example.test/repos/O/R",
    "number": number, "title": format!("Issue {number}"), "state": state,
    "html_url": format!("https://github.com/O/R/issues/{number}"),
  })
}

fn child(fixture: &Fixture, number: u64, state: &str, blocked_by: &[u64]) {
  let blockers: Vec<Value> = blocked_by.iter().map(|n| sub(*n, "open")).collect();
  ok(
    fixture,
    &format!("/repos/O/R/issues/{number}/dependencies/blocked_by"),
    &json!(blockers),
  );
  let mut item = sub(number, state);
  item["body"] = json!("");
  ok(fixture, &format!("/repos/O/R/issues/{number}"), &item);
}

fn record(state: &Path, key: &str, body: &Value) {
  let path = state.join("issues").join(format!("{key}.json"));
  std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
  std::fs::write(path, body.to_string()).expect("record");
}

fn serve_github_epic(fixture: &Fixture) {
  ok(
    fixture,
    "/repos/O/R/issues/1",
    &json!({"title": "Epic", "state": "open", "html_url": "u", "body": ""}),
  );
  let subs = json!([
    sub(2, "open"),
    sub(3, "open"),
    sub(4, "closed"),
    sub(5, "open")
  ]);
  ok(
    fixture,
    "/repos/O/R/issues/1/sub_issues?per_page=100",
    &subs,
  );
  child(fixture, 2, "open", &[]);
  child(fixture, 3, "open", &[2]);
  child(fixture, 4, "closed", &[]);
  child(fixture, 5, "open", &[]);
  ok(
    fixture,
    "/graphql",
    &json!({"data": {"repository": {"issue": null}}}),
  );
}

fn build_github(fixture: &Fixture, tmp: &Path) -> (Graph, PathBuf) {
  serve_github_epic(fixture);
  let checkout = tmp.join("src").join("r");
  repo_with_origin(&checkout, "https://github.com/O/R.git");
  let state = tmp.join("state");
  record(
    &state,
    "o-r-2",
    &json!({"stage": "running", "branch": "feat/2-custom"}),
  );
  let tracker = crate::tracker::github::GitHubTracker::with(
    "O/R#1",
    gh(fixture),
    crate::tracker::github::CLOSE_POLLS,
  )
  .expect("tracker");
  let graph = build(&BuildInput {
    tracker: &tracker,
    default_repo: String::new(),
    state_dir: state,
    max_parallel: 2,
    gh: None,
    herdr_roots: vec![checkout.clone()],
    repo_roots: Vec::new(),
    cwd: tmp.to_path_buf(),
  })
  .expect("graph");
  (graph, checkout)
}

#[test]
fn a_github_epic_builds_statuses_batch_and_checkouts() {
  let fixture = Fixture::start().expect("fixture");
  let tmp = tempfile::tempdir().expect("temp");
  let (graph, checkout) = build_github(&fixture, tmp.path());
  let status: Vec<(&str, &str)> = graph
    .issues
    .iter()
    .map(|i| (i.reference.as_str(), i.status.as_str()))
    .collect();
  assert_eq!(
    status,
    [
      ("O/R#2", "in_flight"),
      ("O/R#3", "blocked"),
      ("O/R#4", "done"),
      ("O/R#5", "ready")
    ]
  );
  assert_eq!(graph.launch_batch, ["O/R#5"]);
  assert_eq!(graph.issues[0].branch, "feat/2-custom");
  assert_eq!(graph.issues[1].open_blockers, ["O/R#2"]);
  assert_eq!(graph.issues[1].wave, Some(2));
  assert_eq!(graph.issues[0].unblocks, 1);
  assert_eq!(graph.missing_checkouts, Vec::<String>::new());
  assert_eq!(
    graph.clone_root,
    checkout.parent().expect("parent").display().to_string()
  );
  assert_eq!(graph.tracker.as_deref(), Some("github"));
  assert!(!graph.complete);
  assert_eq!(graph.counts.ready, 1);
}

#[test]
fn a_jira_epic_finds_prs_by_branch_and_reports_missing_checkouts() {
  let fixture = Fixture::start().expect("fixture");
  let issue = |key: &str| json!({"key": key, "fields": {"summary": key, "status": {"statusCategory": {"key": "new"}}, "labels": ["repo:acme/refunds"]}});
  fixture
    .route(
      "/rest/api/3/issue/PAY-7?fields=summary,status",
      toolu_http_test_support::Reply::new(200, issue("PAY-7").to_string()),
    )
    .expect("route");
  ok(
    &fixture,
    "/rest/api/3/search/jql",
    &json!({"issues": [issue("PAY-12")]}),
  );
  let pulls = json!([{"number": 9, "state": "closed", "merged_at": "2026-01-01T00:00:00Z", "html_url": "pr", "head": {"ref": "feat/pay-12-x"}}]);
  ok(
    &fixture,
    "/repos/acme/refunds/pulls?head=acme:feat/pay-12-x&state=all",
    &pulls,
  );
  let tmp = tempfile::tempdir().expect("temp");
  let state = tmp.path().join("state");
  record(&state, "pay-12", &json!({"branch": "feat/pay-12-x"}));
  let http = crate::tracker::rest::Http::with_client(http_client(&fixture), FAST, "epic jira");
  let client = crate::tracker::jira_client::JiraClient::with_http(&jira_env(&fixture, "3"), http)
    .expect("client");
  let tracker =
    crate::tracker::jira::JiraTracker::with("PAY-7", "acme/mono", client).expect("tracker");
  let github = gh(&fixture);
  let graph = build(&BuildInput {
    tracker: &tracker,
    default_repo: "acme/mono".into(),
    state_dir: state,
    max_parallel: 3,
    gh: Some(&github),
    herdr_roots: Vec::new(),
    repo_roots: Vec::new(),
    cwd: tmp.path().join("here"),
  })
  .expect("graph");
  assert_eq!(graph.issues[0].prs[0].state, "MERGED");
  assert_eq!(graph.issues[0].key, "pay-12");
  assert_eq!(graph.missing_checkouts, ["acme/mono", "acme/refunds"]);
  assert_eq!(graph.default_repo.as_deref(), Some("acme/mono"));
  assert_eq!(graph.clone_root, tmp.path().display().to_string());
  assert_eq!(tracker.kind().name(), "jira");
}
