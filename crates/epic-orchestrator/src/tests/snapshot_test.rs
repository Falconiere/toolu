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
fn deferred_merge_and_rebase_round_trip_through_the_issue_record() {
  for deferred in ["merge", "rebase"] {
    let tmp = tempfile::tempdir().expect("temp");
    let epic = tmp.path().join("epic");
    std::fs::create_dir_all(epic.join("issues")).expect("issues");
    std::fs::create_dir_all(epic.join("status")).expect("status");
    std::fs::write(
      epic.join("graph.json"),
      json!({"issues": [{"key": "a", "repo": "o/r"}]}).to_string(),
    )
    .expect("graph");
    std::fs::write(
      tmp.path().join("registry.json"),
      json!({"epics": [{"key": "e", "state_dir": epic.display().to_string()}]}).to_string(),
    )
    .expect("registry");
    std::fs::write(epic.join("status/a.json"), r#"{"phase":"ready","pr":7}"#).expect("status");
    let mut issue = crate::model::Issue::blank("a", "e", "", 0);
    issue.stage = "running".into();
    issue.deferred = Some(deferred.into());
    crate::disk::write_issue_fields(&epic.join("issues/a.json"), &issue).expect("record");
    let world = load(&Paths::at(tmp.path()), std::time::SystemTime::now()).expect("load");
    assert_eq!(world.issues["a"].deferred.as_deref(), Some(deferred));
    assert!(
      world.issues["a"].pending.is_none(),
      "restart must not rearm a held write"
    );
  }
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

#[test]
fn a_refresh_drops_an_issue_that_left_the_graph_and_frees_its_slot() {
  let fix = crate::source_fix::fixture(true).expect("fixture");
  let mut engine =
    crate::server::Engine::open(fix.engine_paths.clone(), None, crate::server::Fault::None)
      .expect("open");
  let graph = fix.tmp.path().join("epic").join("graph.json");
  let taken = |world: &crate::model::World| {
    world
      .issues
      .values()
      .filter(|issue| issue.epic == "one" && crate::refill::occupies(issue))
      .count()
  };
  assert_eq!(taken(&engine.world), 1);
  // A graph without an issue list drops nothing.
  std::fs::write(&graph, r#"{"epic":"one"}"#).expect("graph");
  super::reload_epic(&mut engine.world, "one").expect("reload");
  assert!(engine.world.issues.contains_key("a"));
  std::fs::write(&graph, r#"{"epic":"one","issues":[]}"#).expect("graph");
  engine.world.issues.get_mut("a").expect("a").pending = Some(crate::model::fresh(
    crate::model::Action::Checkpoint,
    "t".to_owned(),
  ));
  super::reload_epic(&mut engine.world, "one").expect("reload");
  assert!(
    engine.world.issues.contains_key("a"),
    "a pending action keeps it"
  );
  engine.world.issues.get_mut("a").expect("a").pending = None;
  super::reload_epic(&mut engine.world, "one").expect("reload");
  assert!(!engine.world.issues.contains_key("a"));
  assert_eq!(taken(&engine.world), 0);
  let dropped = engine.world.outbox.iter().any(|step| {
    matches!(step, crate::model::Step::Journal(row) if row.name == "drop" && row.key == "a" && row.note == "not in the graph")
  });
  assert!(dropped);
}

#[test]
fn a_graph_issue_done_in_the_tracker_is_marked_and_a_reopen_clears_it() {
  let tmp = tempfile::tempdir().expect("temp");
  let epic = tmp.path().join("epic");
  std::fs::create_dir_all(epic.join("issues")).expect("issues");
  let graph = |status: &str| json!({"issues": [{"key": "a", "status": status}]}).to_string();
  std::fs::write(epic.join("graph.json"), graph("done")).expect("graph");
  let registry = json!({
    "version": 1,
    "epics": [{"key": "one", "state_dir": epic.display().to_string()}]
  });
  std::fs::write(tmp.path().join("registry.json"), registry.to_string()).expect("registry");
  let mut world = load(&Paths::at(tmp.path()), std::time::SystemTime::now()).expect("load");
  assert!(world.issues["a"].tracker_done);
  assert!(!crate::refill::ready(&world, &world.issues["a"]));
  std::fs::write(epic.join("graph.json"), graph("ready")).expect("graph");
  super::reload_epic(&mut world, "one").expect("reload");
  assert!(!world.issues["a"].tracker_done);
}
