use toolu_engine::resources::leases::{LeaseKind, LeaseRequest, acquire_lease};

use crate::launch::{find_issue, launch_issue};
use crate::launch_rig::{Rig, opts};

#[test]
fn shared_capacity_refuses_before_external_work() {
  let rig = Rig::new();
  let root = rig.tmp.path().join("resources");
  std::fs::create_dir_all(&root).expect("root");
  std::fs::write(root.join("policy.json"), r#"{"maxAgents":1}"#).expect("policy");
  let request = LeaseRequest {
    kind: LeaseKind::Agent,
    key: "other".into(),
    state_dir: "another-epic".into(),
    host: Some("claude".into()),
    host_cap: None,
    epic_cap: None,
    worktree: None,
  };
  acquire_lease(&root, &request, std::time::SystemTime::now()).expect("other");
  let mut flags = opts();
  flags.force = true;
  let issue = find_issue(&rig.graph, "comemory-255")
    .expect("issue")
    .clone();
  let refused = launch_issue(&rig.graph, &issue, &flags, &mut rig.ctx()).expect_err("capacity");
  assert!(
    refused.message.contains("capacity exhausted"),
    "{}",
    refused.message
  );
  assert_eq!(rig.leases(), 1);
  assert_eq!(rig.herdr.calls(), Vec::new());
}

#[test]
fn a_dry_resume_prompts_with_the_captured_session() {
  let rig = Rig::new();
  rig.write_record("comemory-255", &serde_json::json!({"stage": "running", "kind": "opencode", "launches": 1, "session_id": "ses_abc"}));
  let mut flags = opts();
  flags.dry_run = true;
  flags.kind = Some("opencode".into());
  flags.model = Some("p/m".into());
  let issue = find_issue(&rig.graph, "comemory-255")
    .expect("issue")
    .clone();
  let result = launch_issue(&rig.graph, &issue, &flags, &mut rig.ctx()).expect("dry resume");
  assert_eq!(result["resume"], true);
  assert_eq!(
    result["agent_args"],
    serde_json::json!(["--session", "ses_abc", "--auto", "--model", "p/m"])
  );
  assert!(result["prompt"].as_str().is_some_and(|prompt| {
    prompt.contains("resuming interrupted work") && prompt.ends_with("loaded above.")
  }));
}

#[test]
fn reprompt_takes_over_an_uncertain_attempt_and_replace_a_replacing_one() {
  let rig = Rig::new();
  let issue = find_issue(&rig.graph, "comemory-255")
    .expect("issue")
    .clone();
  let mut flags = opts();
  flags.dry_run = true;
  for (stage, recovery) in [
    ("uncertain", crate::launch::Recovery::Reprompt),
    ("replacing", crate::launch::Recovery::Replace),
  ] {
    rig.write_record("comemory-255", &serde_json::json!({"stage": stage}));
    flags.recovery = recovery;
    let result = launch_issue(&rig.graph, &issue, &flags, &mut rig.ctx()).expect(stage);
    let commands = result["commands"].as_array().cloned().unwrap_or_default();
    assert!(commands.iter().any(|line| {
      line
        .as_str()
        .is_some_and(|line| line.starts_with("herdr agent prompt comemory-255"))
    }));
  }
}
