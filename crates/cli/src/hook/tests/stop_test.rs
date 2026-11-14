//! A corrupt result stops the host with a named diagnostic.

use toolu_protocol::exit::Exit;
use toolu_runtime::env::Env;

use super::{HookCall, run};
use crate::fast::HookRequest;

#[test]
fn an_invalid_result_blocks_instead_of_disappearing() {
  let dir = tempfile::tempdir().unwrap();
  let project = dir.path().join("project");
  std::fs::create_dir(&project).unwrap();
  assert!(
    std::process::Command::new("git")
      .args(["init", "-q"])
      .current_dir(&project)
      .status()
      .unwrap()
      .success()
  );
  let root = dir.path().join("resources");
  std::fs::create_dir_all(root.join("jobs/results")).unwrap();
  std::fs::write(root.join("jobs/results/bad.json"), "not-json").unwrap();
  let env = Env::from_pairs([
    ("HOME", dir.path().display().to_string()),
    ("PATH", std::env::var("PATH").unwrap()),
    ("TOOLU_RESOURCE_HOME", root.display().to_string()),
  ]);
  let request = HookRequest {
    plugin: "toolu".to_owned(),
    name: "stop".to_owned(),
    event: Some("Stop".to_owned()),
    plugin_root: None,
  };
  let stdin = || Ok(String::new());
  let cwd = || Ok(project.clone());
  let call = HookCall {
    request: &request,
    stdin: &stdin,
    exe: None,
    advisory: None,
    env: &env,
    cwd: &cwd,
  };
  let result = run(&call);
  assert_eq!(result.exit, Exit::Blocked);
  assert!(result.stderr.unwrap().contains("bad.json"));
}
