use serde_json::json;

use super::load;
use crate::paths::Paths;

#[test]
fn loads_blockers_from_the_graph() {
  let tmp = tempfile::tempdir().expect("temp");
  let epic = tmp.path().join("epic");
  std::fs::create_dir_all(epic.join("issues")).expect("issues");
  let graph = json!({
    "issues": [
      {"key": "a", "open_blockers": []},
      {"key": "b", "open_blockers": ["a"]}
    ]
  });
  std::fs::write(epic.join("graph.json"), graph.to_string()).expect("graph");
  let registry = json!({
    "version": 1,
    "epics": [{"key": "one", "state_dir": epic.display().to_string()}]
  });
  std::fs::write(tmp.path().join("registry.json"), registry.to_string()).expect("registry");
  // A relaunch that was waiting when the engine stopped is written to the
  // record and comes back with it.
  let mut waiting = crate::model::Issue::blank("a", "one", "", 0);
  waiting.stage = "running".into();
  waiting.relaunch = Some("reroute".into());
  crate::disk::write_issue_fields(&epic.join("issues/a.json"), &waiting).expect("record");
  let world = load(&Paths::at(tmp.path()), std::time::SystemTime::now()).expect("load");
  let blocked = world.issues.get("b").expect("b");
  assert_eq!(blocked.blockers, vec!["a".to_owned()]);
  assert_eq!(blocked.epic, "one");
  assert_eq!(world.issues["a"].relaunch.as_deref(), Some("reroute"));
  assert_eq!(blocked.relaunch, None);
}

#[test]
fn adopt_typescript() {
  let tmp = tempfile::tempdir().expect("temp");
  let epic = tmp.path().join("epic");
  std::fs::create_dir_all(epic.join("issues")).expect("issues");
  std::fs::create_dir_all(epic.join("status")).expect("status");
  let state = epic.display().to_string();
  std::fs::write(
    epic.join("graph.json"),
    json!({"issues": [{"key": "a", "open_blockers": []}]}).to_string(),
  )
  .expect("graph");
  std::fs::write(
    epic.join("issues").join("a.json"),
    json!({"stage": "running"}).to_string(),
  )
  .expect("issue");
  let status = epic.join("status").join("a.json");
  std::fs::write(
    &status,
    json!({"phase": "starting", "agent": "kept"}).to_string(),
  )
  .expect("status");
  std::fs::write(
    tmp.path().join("registry.json"),
    json!({"version": 1, "epics": [{"key": "one", "state_dir": state}]}).to_string(),
  )
  .expect("registry");
  let mut engine =
    crate::server::Engine::open(Paths::at(tmp.path()), None, crate::server::Fault::None)
      .expect("open");
  engine
    .report(&crate::model::Report {
      key: "a".to_owned(),
      epic: "one".to_owned(),
      state_dir: epic.display().to_string(),
      phase: "ready".to_owned(),
      pr: None,
      note: String::new(),
    })
    .expect("report");
  assert_keeps_agent(&status);
}

/// The report rewrote the phase and kept the field another writer added.
fn assert_keeps_agent(status: &std::path::Path) {
  let doc: serde_json::Value =
    serde_json::from_str(&std::fs::read_to_string(status).expect("status")).expect("json");
  assert_eq!(
    (doc["phase"].as_str(), doc["agent"].as_str()),
    (Some("ready"), Some("kept"))
  );
}
