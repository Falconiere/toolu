use std::collections::BTreeSet;

use super::{OCCUPIED_STAGES, classify, occupies_slot};
use crate::support::graph_248;

#[test]
fn ready_blocked_done_and_in_flight() {
  let graph = graph_248();
  let refs: BTreeSet<&str> = graph
    .issues
    .iter()
    .map(|issue| issue.reference.as_str())
    .collect();
  let find = |reference: &str| graph.find(reference).expect("issue");
  assert_eq!(
    classify(find("Falconiere/comemory#255"), &refs, None),
    "ready"
  );
  assert_eq!(
    classify(find("Falconiere/comemory#257"), &refs, None),
    "blocked"
  );
  assert_eq!(
    classify(find("Falconiere/comemory#253"), &refs, None),
    "done"
  );
  assert_eq!(
    classify(find("Falconiere/comemory#255"), &refs, Some("running")),
    "in_flight"
  );
  assert_eq!(
    classify(find("Falconiere/comemory#255"), &refs, Some("merged")),
    "ready"
  );
}

#[test]
fn durable_ownership_stays_in_flight_after_the_tracker_issue_closes() {
  let graph = graph_248();
  let refs: BTreeSet<&str> = graph
    .issues
    .iter()
    .map(|issue| issue.reference.as_str())
    .collect();
  let closed = graph.find("Falconiere/comemory#253").expect("253");
  for stage in OCCUPIED_STAGES {
    assert_eq!(classify(closed, &refs, Some(stage)), "in_flight", "{stage}");
  }
  for stage in ["merged", "abandoned", "unknown"] {
    assert_eq!(classify(closed, &refs, Some(stage)), "done", "{stage}");
  }
  assert!(!occupies_slot(None));
}

#[test]
fn a_blocker_outside_the_epic_is_external_and_an_open_pr_is_in_flight() {
  let graph = graph_248();
  let refs: BTreeSet<&str> = graph
    .issues
    .iter()
    .map(|issue| issue.reference.as_str())
    .collect();
  let mut issue = graph.find("Falconiere/comemory#254").expect("254").clone();
  issue
    .blockers
    .insert("Falconiere/other#9".into(), "open".into());
  assert_eq!(classify(&issue, &refs, None), "external_blocked");
  let mut open_pr = graph.find("Falconiere/comemory#255").expect("255").clone();
  open_pr.prs.push(crate::tracker::PrNode {
    number: 1,
    state: "OPEN".into(),
    url: String::new(),
    head_ref_name: String::new(),
  });
  assert_eq!(classify(&open_pr, &refs, None), "in_flight");
}
