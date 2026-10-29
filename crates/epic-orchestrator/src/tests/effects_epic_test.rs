use toolu_http_test_support::{Fixture, Reply};
use toolu_runtime::env::Env;

use super::{EpicJob, run};
use crate::graph::write_graph;
use crate::model::Action;
use crate::paths::Paths;
use crate::support::{FAST, graph_248, reach};

fn epic_job(fixture: &Fixture, dir: &std::path::Path) -> EpicJob {
  EpicJob {
    state_dir: dir.to_path_buf(),
    reference: "O/R#1".into(),
    tracker: Some("github".into()),
    default_repo: String::new(),
    max_parallel: 2,
    env: Env::from_pairs([("HOME", dir.display().to_string())]),
    paths: Paths::at(dir),
    reach: reach(fixture, FAST),
  }
}

#[test]
fn a_recovered_close_retries_the_transition_without_another_comment() {
  let fixture = Fixture::start().expect("fixture");
  let tmp = tempfile::tempdir().expect("temp");
  let mut graph = graph_248();
  graph.epic.reference = "O/R#1".into();
  graph.state_dir = tmp.path().display().to_string();
  graph.complete = true;
  write_graph(&tmp.path().join("graph.json"), &graph).expect("graph");
  serve_close(&fixture);
  assert_eq!(
    run(Action::Close, &epic_job(&fixture, tmp.path()))["outcome"],
    "failed"
  );
  assert_eq!(
    run(Action::Close, &epic_job(&fixture, tmp.path()))["outcome"],
    "applied"
  );
  let requests = fixture.requests().expect("requests");
  assert_eq!(
    requests
      .iter()
      .filter(|request| request.method == "POST")
      .count(),
    1
  );
  assert_eq!(
    requests
      .iter()
      .filter(|request| request.method == "PATCH")
      .count(),
    2
  );
}

fn serve_close(fixture: &Fixture) {
  fixture
    .route(
      "/repos/O/R/issues/1",
      Reply::new(200, r#"{"state":"open"}"#),
    )
    .expect("issue");
  fixture
    .sequence_when(
      "/repos/O/R/issues/1",
      "\"state\":\"closed\"",
      vec![Reply::new(503, "{}"), Reply::new(200, "{}")],
    )
    .expect("transition");
  fixture
    .route(
      "/repos/O/R/issues/1/comments",
      Reply::new(201, r#"{"id":1}"#),
    )
    .expect("comment");
  fixture
    .route(
      "/repos/O/R/issues/1/comments?per_page=100",
      Reply::new(200, "[]"),
    )
    .expect("list");
}

#[test]
fn a_close_without_a_graph_and_a_non_epic_action_fail() {
  let fixture = Fixture::start().expect("fixture");
  let tmp = tempfile::tempdir().expect("temp");
  let doc = run(Action::Close, &epic_job(&fixture, tmp.path()));
  assert_eq!(doc["outcome"], "failed");
  assert!(
    doc["note"]
      .as_str()
      .is_some_and(|note| note.contains("cannot read graph"))
  );
  let doc = run(Action::Merge, &epic_job(&fixture, tmp.path()));
  assert_eq!(doc["note"], "merge is not an epic action");
  assert_eq!(fixture.requests().expect("requests").len(), 0);
}
