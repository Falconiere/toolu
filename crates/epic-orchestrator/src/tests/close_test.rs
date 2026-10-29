use serde_json::{Value, json};
use toolu_http_test_support::{Fixture, Reply};

use super::{close_epic, require_complete, summary_table};
use crate::graph::Graph;
use crate::support::{gh, graph_248};
use crate::tracker::PrNode;
use crate::tracker::github::{CLOSE_POLLS, GitHubTracker};

fn graph(dir: &std::path::Path) -> Graph {
  let mut graph = graph_248();
  graph.state_dir = dir.display().to_string();
  graph
}

#[test]
fn the_summary_lists_every_sub_issue_with_its_merged_pr_and_admin_merges() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut graph = graph(tmp.path());
  let first = graph.issues[0].key.clone();
  graph.issues[0].prs = vec![PrNode {
    number: 7,
    state: "MERGED".into(),
    url: "https://github.com/x/y/pull/7".into(),
    head_ref_name: "b".into(),
  }];
  graph.issues[1].title = "a | b".into();
  let record = tmp.path().join("issues").join(format!("{first}.json"));
  std::fs::create_dir_all(record.parent().expect("parent")).expect("dir");
  std::fs::write(
    &record,
    json!({"last_gate": {"admin_used": true}}).to_string(),
  )
  .expect("record");
  let text = summary_table(&graph, None);
  let lines: Vec<&str> = text.lines().collect();
  assert_eq!(
    lines[..4],
    [
      "Epic complete.",
      "",
      "| Sub-issue | Merged PR |",
      "|---|---|"
    ]
  );
  assert_eq!(lines.len(), 4 + graph.issues.len());
  assert!(text.contains("https://github.com/x/y/pull/7 (admin merge)"));
  assert!(text.contains("closed without a merged PR") && text.contains("a \\| b"));
}

#[test]
fn an_incomplete_epic_is_refused_with_its_open_refs() {
  let tmp = tempfile::tempdir().expect("temp");
  let graph = graph(tmp.path());
  let err = require_complete(&graph).expect_err("incomplete");
  assert!(
    err.starts_with(
      "epic not complete; still open: Falconiere/comemory#254, Falconiere/comemory#255"
    ),
    "{err}"
  );
  assert!(err.ends_with(". Re-run the graph first."));
}

#[test]
fn a_complete_epic_is_summarized_and_closed_in_github() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut graph = graph(tmp.path());
  graph.complete = true;
  let fixture = Fixture::start().expect("fixture");
  for path in [
    "/repos/Falconiere/comemory/issues/248/comments",
    "/repos/Falconiere/comemory/issues/248",
  ] {
    fixture.route(path, Reply::new(200, "{}")).expect("route");
  }
  fixture
    .route(
      "/repos/Falconiere/comemory/issues/248/comments?per_page=100",
      Reply::new(200, "[]"),
    )
    .expect("comments");
  let tracker =
    GitHubTracker::with("Falconiere/comemory#248", gh(&fixture), CLOSE_POLLS).expect("tracker");
  let metrics = "## Epic metrics\n\n| Measure | This epic | TypeScript baseline |\n";
  let result = close_epic(&graph, &tracker, Some(metrics)).expect("closed");
  assert_eq!(
    result,
    json!({"epic": "Falconiere/comemory#248", "tracker": "github", "result": "closed"})
  );
  let requests = fixture.requests().expect("requests");
  let comment_request = requests
    .iter()
    .find(|request| request.method == "POST")
    .expect("comment request");
  let comment: Value = serde_json::from_slice(&comment_request.body).expect("comment");
  assert!(
    comment["body"]
      .as_str()
      .is_some_and(|body| body.starts_with("Epic complete."))
  );
  let body = comment["body"].as_str().unwrap_or("");
  assert!(
    body.ends_with("\n\n## Epic metrics\n\n| Measure | This epic | TypeScript baseline |"),
    "{body}"
  );
  let close_request = requests
    .iter()
    .find(|request| request.method == "PATCH")
    .expect("close request");
  let closed: Value = serde_json::from_slice(&close_request.body).expect("close");
  assert_eq!(
    closed,
    json!({"state": "closed", "state_reason": "completed"})
  );
}

#[test]
fn closing_an_incomplete_epic_makes_no_request() {
  let tmp = tempfile::tempdir().expect("temp");
  let fixture = Fixture::start().expect("fixture");
  let tracker =
    GitHubTracker::with("Falconiere/comemory#248", gh(&fixture), CLOSE_POLLS).expect("tracker");
  assert!(close_epic(&graph(tmp.path()), &tracker, None).is_err());
  assert_eq!(fixture.requests().expect("requests").len(), 0);
}
