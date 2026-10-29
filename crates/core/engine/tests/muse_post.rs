//! Muse `PostToolUse` status and the following pre-tool quality decision.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::Duration;

use serde_json::{Value, json};
use toolu_engine::builtins::{POST_TOOL, PRE_TOOL};
use toolu_engine::{DispatchOptions, dispatch_post_tool, dispatch_pre_tool};
use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;

struct CargoSite {
  _dir: tempfile::TempDir,
  project: PathBuf,
  config: PathBuf,
  env: Env,
}

impl CargoSite {
  fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync + 'static>> {
    let dir = tempfile::tempdir()?;
    let project = dir.path().join("project");
    let config = dir.path().join("config");
    fs::create_dir_all(project.join("src"))?;
    fs::create_dir_all(&config)?;
    fs::write(
      project.join("Cargo.toml"),
      "[package]\nname = \"muse-gate-case\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[workspace]\n",
    )?;
    fs::write(
      config.join("toolu.config.json"),
      json!({"version":1,"gates":{"qualityGate":{"mode":"block"}}}).to_string(),
    )?;
    let init = Command::new("git")
      .arg("init")
      .arg("-b")
      .arg("main")
      .arg(&project)
      .output()?;
    if !init.status.success() {
      return Err(format!("git init failed: {init:?}").into());
    }
    let env = Env::from_pairs([
      ("HOME", dir.path().to_string_lossy().into_owned()),
      ("PATH", std::env::var("PATH")?),
      ("MUSE_PLUGIN_ID", "toolu".to_owned()),
      ("TOOLU_CONFIG_DIR", config.to_string_lossy().into_owned()),
    ]);
    Ok(Self {
      _dir: dir,
      project,
      config,
      env,
    })
  }

  fn options(&self, post: bool) -> DispatchOptions<'_> {
    DispatchOptions {
      env: &self.env,
      cwd: &self.project,
      lib_dir: &self.config,
      builtins: if post { POST_TOOL } else { PRE_TOOL },
      rules: &[],
      quality_rules: &[],
      selected_specs: None,
      continue_post_blocks: false,
      split_by_caller: false,
      module_timeout: Duration::from_secs(1),
    }
  }

  fn cargo_test(&self) -> std::io::Result<Output> {
    Command::new("cargo")
      .args(["test", "--offline"])
      .current_dir(&self.project)
      .env("CARGO_TARGET_DIR", self.project.join("target"))
      .output()
  }

  fn payload(&self, command: &str, result: Option<&Output>) -> String {
    let mut payload = json!({
      "hook_event_name":if result.is_some() { "PostToolUse" } else { "PreToolUse" },
      "session_id":"s1", "cwd":self.project, "tool_name":"bash",
      "tool_input":{"command":command}
    });
    if let Some(result) = result {
      let response = json!({
        "exit_code":result.status.code(),
        "output":String::from_utf8_lossy(&result.stdout).to_string(),
        "stderr":String::from_utf8_lossy(&result.stderr).to_string()
      })
      .to_string();
      if let Some(fields) = payload.as_object_mut() {
        fields.insert("tool_response".to_owned(), response.into());
      }
    }
    payload.to_string()
  }

  fn gate_file(&self) -> Option<PathBuf> {
    Roots::new(self.env.clone(), Some(Host::Muse))
      .project_state_root(None, Some(&self.project))
      .map(|root| root.join("quality-gate-status.json"))
  }
}

#[test]
fn real_cargo_failure_blocks_muse_commit_until_a_passing_run() {
  let site = CargoSite::new().expect("sandbox");
  let source = site.project.join("src/lib.rs");
  fs::write(&source, "#[test] fn check() { assert_eq!(1, 2); }\n").expect("failing test");
  let failed = site.cargo_test().expect("failing cargo test");
  assert!(!failed.status.success(), "test should fail");
  let command = "cargo test --offline";
  let post = dispatch_post_tool(&site.payload(command, Some(&failed)), &site.options(true));
  assert_eq!(post.result.exit_code, 0, "{post:?}");
  let gate =
    fs::read_to_string(site.gate_file().expect("state root")).expect("recorded quality failure");
  assert!(gate.contains("\"status\": \"failing\""), "{gate}");
  let commit = site.payload("git commit -m 'feat: change'", None);
  let blocked = dispatch_pre_tool(&commit, &site.options(false));
  let decision: Value = serde_json::from_str(&blocked.result.stdout).expect("quality decision");
  assert_eq!(decision["decision"], "block", "{blocked:?}");
  assert!(
    decision["reason"]
      .as_str()
      .is_some_and(|reason| reason.contains("quality gate failing"))
  );

  fs::write(&source, "#[test] fn check() { assert_eq!(2, 2); }\n").expect("fixed test");
  let passed = site.cargo_test().expect("passing cargo test");
  assert!(passed.status.success(), "{passed:?}");
  let post = dispatch_post_tool(&site.payload(command, Some(&passed)), &site.options(true));
  assert_eq!(post.result.exit_code, 0, "{post:?}");
  let gate =
    fs::read_to_string(site.gate_file().expect("state root")).expect("cleared quality failure");
  assert!(!gate.contains("\"status\": \"failing\""), "{gate}");
  let unblocked = dispatch_pre_tool(&commit, &site.options(false));
  let decision: Value =
    serde_json::from_str(&unblocked.result.stdout).expect("valid Muse advisory");
  assert_eq!(unblocked.result.exit_code, 0, "{unblocked:?}");
  assert!(decision["systemMessage"].is_string(), "{unblocked:?}");
}
