use super::{fixture, in_repo};

#[test]
fn in_repo_records_the_issue_repository() {
  let fix = fixture(false).expect("fixture");
  in_repo(&fix).expect("graph");
  let graph = std::fs::read_to_string(fix.tmp.path().join("epic/graph.json")).expect("read");
  assert_eq!(
    graph,
    r#"{"epic":"one","issues":[{"key":"a","repo":"o/r","open_blockers":[]}]}"#
  );
}
