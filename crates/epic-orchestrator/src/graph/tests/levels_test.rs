use std::collections::BTreeMap;

use super::{chain_lengths, compute_levels, downstream_counts};
use crate::support::graph_248;

#[test]
fn waves_follow_open_blockers() {
  let graph = graph_248();
  let (levels, cycle) = compute_levels(&graph.issues);
  assert_eq!(cycle, Vec::<String>::new());
  let expected: BTreeMap<String, u64> = [
    ("Falconiere/comemory#254", 1),
    ("Falconiere/comemory#255", 1),
    ("CodaSignal/comemory.io#183", 1),
    ("Falconiere/comemory#256", 2),
    ("Falconiere/comemory#257", 2),
    ("CodaSignal/comemory.io#184", 2),
    ("Falconiere/comemory#258", 3),
    ("CodaSignal/comemory.io#185", 3),
    ("Falconiere/homebrew-tap#1", 4),
  ]
  .into_iter()
  .map(|(reference, level)| (reference.to_owned(), level))
  .collect();
  assert_eq!(levels, expected);
}

#[test]
fn the_critical_path_issue_unblocks_most() {
  let counts = downstream_counts(&graph_248().issues);
  assert_eq!(counts["Falconiere/comemory#255"], 5);
  assert_eq!(counts["Falconiere/comemory#254"], 2);
  assert_eq!(counts["CodaSignal/comemory.io#183"], 2);
  assert_eq!(counts["Falconiere/homebrew-tap#1"], 0);
}

#[test]
fn a_cycle_is_reported_and_not_scheduled() {
  let mut graph = graph_248();
  let issue = graph
    .issues
    .iter_mut()
    .find(|issue| issue.reference == "Falconiere/comemory#255")
    .expect("255");
  issue
    .blockers
    .insert("Falconiere/comemory#257".into(), "open".into());
  let (levels, cycle) = compute_levels(&graph.issues);
  assert!(cycle.contains(&"Falconiere/comemory#255".to_owned()));
  assert!(cycle.contains(&"Falconiere/comemory#257".to_owned()));
  assert_eq!(levels, BTreeMap::new());
  let chains = chain_lengths(&graph.issues);
  assert!(chains["Falconiere/comemory#255"] >= 2);
}

#[test]
fn chains_count_the_longest_open_dependent_path() {
  let chains = chain_lengths(&graph_248().issues);
  assert_eq!(chains["Falconiere/homebrew-tap#1"], 1);
  assert!(chains["Falconiere/comemory#255"] > chains["Falconiere/comemory#258"]);
}
