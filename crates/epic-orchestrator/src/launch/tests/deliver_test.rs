use toolu_engine::resources::leases::{LeaseKind, LeaseRequest, acquire_lease};

use serde_json::json;

use crate::launch::{find_issue, launch_issue};
use crate::launch_rig::{GROK_SESSION, Rig, opts};

/// A real checkout, a herdr socket and an agent shell that writes Grok
/// Build's own session layout: the launch records the session it wrote.
#[test]
fn a_live_grok_launch_records_its_grok_session() {
  let mut rig = Rig::new();
  rig.checkout("comemory-255");
  rig.grok_agents();
  let mut flags = opts();
  flags.force = true;
  flags.kind = Some("grok".into());
  rig
    .launch(&flags)
    .unwrap_or_else(|err| panic!("{}", err.message));
  let record = rig.record("comemory-255");
  let fields = ["stage", "kind", "session_id"].map(|key| &record[key]);
  assert_eq!(
    fields,
    [&json!("running"), &json!("grok"), &json!(GROK_SESSION)]
  );
  let calls = rig.herdr.calls();
  let start = calls
    .iter()
    .find(|(method, _)| method == "agent.start")
    .map(|(_, params)| params)
    .expect("agent.start");
  assert_eq!(start["kind"], "grok");
  assert_eq!(start["args"][0], "--always-approve");
}

/// A Grok agent that writes no session cannot be attested.
#[test]
fn a_grok_launch_without_a_session_is_not_attested() {
  let mut rig = Rig::new();
  rig.checkout("comemory-255");
  let mut flags = opts();
  flags.force = true;
  flags.kind = Some("grok".into());
  let failed = rig.launch(&flags).expect_err("no session");
  assert!(
    failed
      .message
      .contains("cannot attest grok session identity"),
    "{}",
    failed.message
  );
}

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
  rig.write_record("comemory-255", &serde_json::json!({"stage": "running", "kind": "claude", "launches": 1, "session_id": "0f8fad5b-d9cb-469f-a165-70867728950e"}));
  let mut flags = opts();
  flags.dry_run = true;
  flags.kind = Some("claude".into());
  flags.model = Some("opus".into());
  let issue = find_issue(&rig.graph, "comemory-255")
    .expect("issue")
    .clone();
  let result = launch_issue(&rig.graph, &issue, &flags, &mut rig.ctx()).expect("dry resume");
  assert_eq!(result["resume"], true);
  let settings = rig
    .state()
    .join("claude-settings.json")
    .display()
    .to_string();
  assert_eq!(
    result["agent_args"],
    serde_json::json!([
      "--resume",
      "0f8fad5b-d9cb-469f-a165-70867728950e",
      "--dangerously-skip-permissions",
      "-n",
      "comemory-255",
      "--model",
      "opus",
      "--settings",
      settings
    ])
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
