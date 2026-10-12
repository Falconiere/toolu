use serde_json::json;
use toolu_http_test_support::Fixture;
use toolu_runtime::env::Env;

use super::{EpicJob, run};
use crate::model::Action;
use crate::paths::Paths;
use crate::support::{FAST, ok, reach};

fn epic_job(fixture: &Fixture, dir: &std::path::Path, recovered: bool) -> EpicJob {
  EpicJob {
    state_dir: dir.to_path_buf(),
    reference: "O/R#1".into(),
    tracker: Some("github".into()),
    default_repo: String::new(),
    max_parallel: 2,
    recovered,
    env: Env::from_pairs([("HOME", dir.display().to_string())]),
    paths: Paths::at(dir),
    reach: reach(fixture, FAST),
  }
}

#[test]
fn a_recovered_close_asks_the_tracker_and_never_comments() {
  let fixture = Fixture::start().expect("fixture");
  let tmp = tempfile::tempdir().expect("temp");
  for (state, outcome) in [("closed", "closed"), ("open", "open")] {
    ok(
      &fixture,
      "/repos/O/R/issues/1",
      &json!({"title": "Epic", "state": state, "html_url": "u", "body": ""}),
    );
    let doc = run(Action::Close, &epic_job(&fixture, tmp.path(), true));
    assert_eq!(doc["outcome"], outcome);
    assert_eq!(doc["epic"], "O/R#1");
  }
  let requests = fixture.requests().expect("requests");
  assert!(requests.iter().all(|request| request.method == "GET"));
}

#[test]
fn a_close_without_a_graph_and_a_non_epic_action_fail() {
  let fixture = Fixture::start().expect("fixture");
  let tmp = tempfile::tempdir().expect("temp");
  let doc = run(Action::Close, &epic_job(&fixture, tmp.path(), false));
  assert_eq!(doc["outcome"], "failed");
  assert!(
    doc["note"]
      .as_str()
      .is_some_and(|note| note.contains("cannot read graph"))
  );
  let doc = run(Action::Merge, &epic_job(&fixture, tmp.path(), false));
  assert_eq!(doc["note"], "merge is not an epic action");
  assert_eq!(fixture.requests().expect("requests").len(), 0);
}
