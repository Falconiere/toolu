use super::pick_batch;
use crate::graph::GraphIssue;
use crate::support::graph_248;

fn issue(reference: &str, repo: &str, number: u64, chain: u64, unblocks: u64) -> GraphIssue {
  let mut base = graph_248().issues[0].clone();
  base.reference = reference.into();
  base.repo = repo.into();
  base.number = Some(number);
  base.chain = chain;
  base.unblocks = unblocks;
  base
}

#[test]
fn the_longest_chain_wins_over_more_unblocked_work() {
  let issues = vec![
    issue("o/r#1", "o/r", 1, 1, 0),
    issue("o/r#2", "o/r", 2, 2, 4),
    issue("o/r#3", "o/r", 3, 3, 2),
  ];
  let ready: Vec<String> = issues.iter().map(|issue| issue.reference.clone()).collect();
  assert_eq!(pick_batch(&ready, &issues, &[], 1), ["o/r#3"]);
  assert_eq!(
    pick_batch(&ready, &issues, &[], 3),
    ["o/r#3", "o/r#2", "o/r#1"]
  );
}

#[test]
fn ties_go_to_the_least_loaded_repository_then_name_then_number() {
  let issues = vec![
    issue("b/x#9", "b/x", 9, 1, 1),
    issue("a/y#5", "a/y", 5, 1, 1),
    issue("a/y#4", "a/y", 4, 1, 1),
    issue("b/x#1", "b/x", 1, 1, 1),
  ];
  let ready: Vec<String> = issues
    .iter()
    .take(3)
    .map(|issue| issue.reference.clone())
    .collect();
  let in_flight = vec!["b/x#1".to_owned()];
  assert_eq!(
    pick_batch(&ready, &issues, &in_flight, 3),
    ["a/y#4", "a/y#5", "b/x#9"]
  );
  assert_eq!(
    pick_batch(&ready, &issues, &in_flight, 0),
    Vec::<String>::new()
  );
}

#[test]
fn the_248_batch_starts_the_critical_path() {
  let graph = graph_248();
  let batch = pick_batch(&graph.ready, &graph.issues, &graph.in_flight, 1);
  assert_eq!(batch, ["Falconiere/comemory#255"]);
}
