use super::{read_graph, write_graph};
use crate::support::graph_248;

#[test]
fn a_graph_round_trips_and_finds_issues_by_ref_key_or_url() {
  let graph = graph_248();
  assert_eq!(
    graph.find("comemory-io-183").map(|i| i.reference.as_str()),
    Some("CodaSignal/comemory.io#183")
  );
  assert!(
    graph
      .find("https://github.com/Falconiere/comemory/issues/255")
      .is_some()
  );
  assert!(graph.find("Falconiere/comemory#999").is_none());
  let tmp = tempfile::tempdir().expect("temp");
  let path = tmp.path().join("state").join("graph.json");
  write_graph(&path, &graph).expect("write");
  assert_eq!(read_graph(&path).expect("read"), graph);
  assert!(
    std::fs::read_to_string(&path)
      .expect("text")
      .ends_with("}\n")
  );
}

#[test]
fn a_missing_or_malformed_graph_is_an_error() {
  let tmp = tempfile::tempdir().expect("temp");
  let missing = tmp.path().join("graph.json");
  assert!(
    read_graph(&missing)
      .expect_err("missing")
      .starts_with("cannot read graph")
  );
  std::fs::write(&missing, "[]").expect("write");
  assert!(read_graph(&missing).is_err());
}
