use super::{configure, implementation_brief, relay_key, saved_request, settled_without_result};
use crate::epics::{LaunchSettings, write_launch_settings};
use crate::launch::LaunchOpts;
use crate::support::graph_248;
use serde_json::{Map, json};
use toolu_runtime::env::Env;

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

#[test]
fn a_settled_relay_task_gets_a_fresh_submission_key() {
  assert_eq!(relay_key("abc", false, 7), "epic-abc");
  assert_eq!(relay_key("abc", true, 7), "epic-abc-7");
  for phase in ["cancelled", "failed"] {
    assert!(settled_without_result(phase));
  }
  for phase in [
    "queued",
    "preparing",
    "running",
    "held",
    "cancelling",
    "done",
  ] {
    assert!(!settled_without_result(phase));
  }
}

#[test]
fn relay_is_used_only_when_started_with_relay_and_one_is_configured() {
  let dir = tempfile::tempdir().unwrap();
  let state = dir.path().join("state");
  let delegation = dir.path().join("config").join("delegation");
  std::fs::create_dir_all(&state).unwrap();
  std::fs::create_dir_all(&delegation).unwrap();
  let env = Env::default().with(
    "TOOLU_CONFIG_DIR",
    dir.path().join("config").to_str().unwrap(),
  );
  let opts = LaunchOpts::default();
  let mut record = Map::new();
  std::fs::write(
    delegation.join("config.json"),
    r#"{"version":1,"relay":"https://relay.test","machines":{}}"#,
  )
  .unwrap();
  write_launch_settings(&state, LaunchSettings::default()).unwrap();
  configure(&mut record, &opts, &state, &env).unwrap();
  assert!(record.is_empty());
  write_launch_settings(
    &state,
    LaunchSettings {
      workers: crate::epics::Workers::Relay,
      ..LaunchSettings::default()
    },
  )
  .unwrap();
  configure(&mut record, &opts, &state, &env).unwrap();
  assert_eq!(record.get("relay_enabled"), Some(&json!(true)));
  assert_eq!(record.get("backend"), Some(&json!("herdr-app")));
  record.clear();
  std::fs::remove_file(delegation.join("config.json")).unwrap();
  let failure = configure(&mut record, &opts, &state, &env).unwrap_err();
  assert!(failure.message.contains("started with --relay"));
  assert!(record.is_empty());
}
