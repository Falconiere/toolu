use super::{implementation_brief, saved_request};
use crate::support::graph_248;
use serde_json::{Map, json};

#[test]
fn recovery_reuses_saved_submission_and_rejects_corrupt_identity() {
  let mut record = Map::new();
  let request = json!({"key":"epic-123", "origin":"https://example.test/repo", "base":"a".repeat(40),
    "brief":"implement the issue", "agent":"codex", "argv":["codex"], "machine":null, "group":null});
  record.insert("relay_request".into(), request.clone());
  assert_eq!(
    serde_json::to_value(saved_request(&record).unwrap().unwrap()).unwrap(),
    request
  );
  record.insert("relay_request".into(), json!({"key":"epic-123"}));
  assert!(saved_request(&record).is_err());
}

#[test]
fn implementation_brief_carries_epic_decisions_and_closed_blockers() {
  let mut graph = graph_248();
  let issue = graph
    .issues
    .iter_mut()
    .find(|issue| issue.reference == "Falconiere/comemory#255")
    .unwrap();
  issue
    .blockers
    .insert("Falconiere/comemory#254".into(), "closed".into());
  issue
    .blockers
    .insert("Falconiere/comemory#256".into(), "open".into());
  let issue = graph.find("Falconiere/comemory#255").unwrap();
  let brief = implementation_brief(
    &graph,
    issue,
    "Agreed decision: preserve the wire format.",
    "Acceptance: support retries.",
    false,
  );
  let request = toolu_mux::relay::store::Submission {
    key: "example".into(),
    origin: "repo".into(),
    base: "a".repeat(40),
    brief,
    agent: "codex".into(),
    argv: vec!["codex".into()],
    machine: None,
    group: None,
  };
  assert!(request.brief.contains(&graph.epic.reference));
  assert!(request.brief.contains(&graph.epic.url));
  assert!(request.brief.contains(&graph.epic.title));
  assert!(request.brief.contains(&issue.reference));
  assert!(request.brief.contains(&issue.url));
  assert!(
    request
      .brief
      .contains("Agreed decision: preserve the wire format.")
  );
  assert!(request.brief.contains("Acceptance: support retries."));
  assert!(request.brief.contains("Falconiere/comemory#254"));
  assert!(!request.brief.contains("Falconiere/comemory#256"));
  assert!(!request.brief.contains("git push"));
  assert!(!request.brief.contains("delivery-flow"));
  assert!(request.brief.contains("AGENTS.md"));
}
