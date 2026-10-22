//! Muse commands and hooks share writable state while project config is read-only (#571).

#[path = "helpers/ledger.rs"]
mod ledger;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::process::Output;

use ledger::{Project, Res};
use serde_json::{Value, json};

struct MuseProject(Project);

impl MuseProject {
  fn new() -> Res<Self> {
    let project = Project::new()?;
    project.write(
      ".agents/toolu.config.json",
      r#"{"version":1,"gates":{"pushReview":{"mode":"block"}}}"#,
    )?;
    for name in [".agents", ".muse"] {
      fs::create_dir_all(project.root.join(name))?;
      fs::set_permissions(project.root.join(name), fs::Permissions::from_mode(0o555))?;
    }
    Ok(Self(project))
  }

  fn run(&self, args: &[&str], payload: &str) -> Res<Output> {
    let mut command = self.0.command(&self.0.root);
    command
      .env_remove("TOOLU_HOST_OVERRIDE")
      .env("MUSE_SESSION_ID", "state-regression")
      .args(args);
    Ok(
      assert_cmd::Command::from_std(command)
        .write_stdin(payload)
        .output()?,
    )
  }

  fn push_gate(&self) -> Res<Output> {
    self.run(
      &["hook", "pre-tools", "--event", "PreToolUse"],
      &json!({"cwd":self.0.root,"tool_name":"bash",
        "tool_input":{"command":"git push origin feat/x"}})
      .to_string(),
    )
  }
}

impl Drop for MuseProject {
  fn drop(&mut self) {
    for name in [".agents", ".muse"] {
      let _ = fs::set_permissions(self.0.root.join(name), fs::Permissions::from_mode(0o755));
    }
  }
}

fn success(output: &Output) -> String {
  assert_eq!(output.status.code(), Some(0), "{output:?}");
  String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn muse_review_writer_and_push_gate_share_state_outside_read_only_config() {
  let project = MuseProject::new().unwrap();
  let blocked: Value = serde_json::from_str(&success(&project.push_gate().unwrap())).unwrap();
  assert_eq!(blocked["decision"], "block", "{blocked}");
  assert!(
    blocked["reason"]
      .as_str()
      .unwrap()
      .contains("Code review required")
  );
  let written = project
    .run(&["review", "write-state", "--findings-count", "0"], "")
    .unwrap();
  let path = project
    .0
    .root
    .join(".toolu/tmp/muse/push-review/feat_x.json");
  assert_eq!(success(&written).trim(), path.display().to_string());
  let state: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
  assert_eq!(state["version"], 2);
  assert_eq!(state["findings_count"], 0);
  let allowed = success(&project.push_gate().unwrap());
  if !allowed.is_empty() {
    let decision: Value = serde_json::from_str(&allowed).unwrap();
    assert!(decision.get("decision").is_none(), "{decision}");
  }
  assert!(!project.0.root.join(".agents/tmp").exists());
  assert!(!project.0.root.join(".muse/tmp").exists());
  assert!(!project.0.ledger().exists());
}

#[test]
fn muse_ledger_runs_and_records_evidence_outside_read_only_config() {
  let project = MuseProject::new().unwrap();
  project
    .0
    .write(
      "plan.md",
      concat!(
        "# Plan\n\n**Status:** Approved   **Spec:** none\n\n",
        "## Steps (machine-readable)\n\n```json\n",
        "[{\"id\":\"git\",\"title\":\"Read Git HEAD\",",
        "\"check\":\"git rev-parse --verify HEAD\"}]\n```\n"
      ),
    )
    .unwrap();
  let path = project
    .0
    .root
    .join(".toolu/tmp/muse/plan-ledger/feat_x.json");
  assert_eq!(
    success(&project.run(&["ledger", "path"], "").unwrap()).trim(),
    path.display().to_string()
  );
  success(
    &project
      .run(&["ledger", "run", "plan.md", "--verify"], "")
      .unwrap(),
  );
  let ledger: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
  assert_eq!(ledger["steps"][0]["status"], "green");
  let head = project.0.sh("git rev-parse HEAD").unwrap();
  assert_eq!(
    ledger["steps"][0]["evidence_tail"].as_str().unwrap().trim(),
    head.trim()
  );
  assert!(ledger["verified_sha"].is_string());
  assert!(!project.0.root.join(".agents/tmp").exists());
  assert!(!project.0.root.join(".muse/tmp").exists());
}
