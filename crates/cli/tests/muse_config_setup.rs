//! Muse config roots and Codex agent setup through the real CLI.

use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;

const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");

fn run(home: &Path, project: &Path, codex: &Path, args: &[&str]) -> Output {
  Command::new(TOOLU)
    .args(["--host", "muse"])
    .args(args)
    .env("HOME", home)
    .env("XDG_CONFIG_HOME", home.join("xdg"))
    .env("TOOLU_PROJECT_DIR", project)
    .env("CODEX_HOME", codex)
    .env_remove("TOOLU_CONFIG_DIR")
    .env_remove("TOOLU_USER_CONFIG_DIR")
    .env_remove("TOOLU_PROJECT_CONFIG_DIRNAME")
    .env_remove("TOOLU_AGENT_TEMPLATE_DIR")
    .env_remove("MUSE_CONFIG_DIR")
    .output()
    .unwrap()
}

#[test]
fn muse_config_uses_xdg_and_project_agents_roots() {
  let root = tempfile::tempdir().unwrap();
  let home = root.path().join("home");
  let project = root.path().join("project");
  let codex = root.path().join("codex");
  std::fs::create_dir_all(&home).unwrap();
  std::fs::create_dir_all(&project).unwrap();

  let user = run(
    &home,
    &project,
    &codex,
    &["config", "set", "gates.pushReview", "\"off\""],
  );
  assert_eq!(user.status.code(), Some(0), "{user:?}");
  let user_path = home.join("xdg/muse/toolu.config.json");
  let user_data: Value = serde_json::from_slice(&std::fs::read(user_path).unwrap()).unwrap();
  assert_eq!(user_data["gates"]["pushReview"], "off");

  let project_set = run(
    &home,
    &project,
    &codex,
    &["config", "set", "gates.pushReview", "\"warn\"", "--project"],
  );
  assert_eq!(project_set.status.code(), Some(0), "{project_set:?}");
  let project_path = project.join(".agents/toolu.config.json");
  let project_data: Value = serde_json::from_slice(&std::fs::read(project_path).unwrap()).unwrap();
  assert_eq!(project_data["gates"]["pushReview"], "warn");

  let get = run(
    &home,
    &project,
    &codex,
    &["config", "get", "gates.pushReview"],
  );
  assert_eq!(get.status.code(), Some(0), "{get:?}");
  assert_eq!(String::from_utf8_lossy(&get.stdout).trim(), "warn");
}

#[test]
fn muse_setup_agents_preview_keeps_codex_home() {
  let root = tempfile::tempdir().unwrap();
  let home = root.path().join("home");
  let project = root.path().join("project");
  let codex = root.path().join("codex");
  std::fs::create_dir_all(&home).unwrap();
  std::fs::create_dir_all(&project).unwrap();
  let output = run(&home, &project, &codex, &["setup", "agents", "preview"]);
  assert_eq!(output.status.code(), Some(0), "{output:?}");
  assert!(
    String::from_utf8_lossy(&output.stdout).contains(&format!("TARGET {}/agents", codex.display()))
  );
  assert!(!codex.exists());
}
