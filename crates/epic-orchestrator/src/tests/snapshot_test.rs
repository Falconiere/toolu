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
  let world = load(&Paths::at(tmp.path()), std::time::SystemTime::now()).expect("load");
  let blocked = world.issues.get("b").expect("b");
  assert_eq!(blocked.blockers, vec!["a".to_owned()]);
  assert_eq!(blocked.epic, "one");
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

fn assert_keeps_agent(status: &std::path::Path) {
  let common = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../../plugins/epic-orchestrator/scripts/common.ts");
  let code = format!(
    "import {{ readJson }} from \"{}\"; const doc = readJson(\"{}\", {{}}); if (doc.phase !== \"ready\" || doc.agent !== \"kept\") throw new Error(JSON.stringify(doc));",
    common.display(),
    status.display()
  );
  let ran = std::process::Command::new("bun")
    .args(["-e", &code])
    .status()
    .expect("bun");
  assert!(ran.success());
}
