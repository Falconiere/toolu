use std::time::SystemTime;

use serde_json::json;
use toolu_engine::resources::leases::{LeaseKind, LeaseRequest, acquire_lease};

use crate::launch::{find_issue, launch_issue};
use crate::launch_rig::{Rig, opts};

#[test]
fn a_failure_after_reservation_leaves_uncertain_ownership_and_refuses_a_blind_retry() {
  let rig = Rig::new();
  let mut flags = opts();
  flags.force = true;
  let issue = find_issue(&rig.graph, "comemory-255")
    .expect("issue")
    .clone();
  let first = launch_issue(&rig.graph, &issue, &flags, &mut rig.ctx()).expect_err("no GitHub");
  let record = rig.record("comemory-255");
  assert_eq!(record["stage"], "uncertain");
  assert_eq!(record["lifecycle_error"], json!(first.message));
  assert!(
    first
      .message
      .starts_with("cannot read the default branch of Falconiere/comemory")
  );
  assert_eq!(record["native_error"]["uncertain"], true);
  assert_eq!(record["ref"], "Falconiere/comemory#255");
  assert_eq!(rig.leases(), 1);
  let retry = launch_issue(&rig.graph, &issue, &flags, &mut rig.ctx()).expect_err("blind retry");
  assert!(
    retry
      .message
      .contains("previous launch/prompt outcome is uncertain")
  );
  assert_eq!(rig.leases(), 1);
}

#[test]
fn incomplete_stages_and_host_changes_refuse_before_ownership() {
  let rig = Rig::new();
  let mut flags = opts();
  flags.force = true;
  let issue = find_issue(&rig.graph, "comemory-255")
    .expect("issue")
    .clone();
  for (stage, recovery) in [
    ("starting", "--reprompt"),
    ("cleaning", "toolu epic finish"),
    ("cleanup-incomplete", "toolu epic finish"),
    ("replacing", "--replace"),
  ] {
    rig.write_record("comemory-255", &json!({"stage": stage}));
    let refused = launch_issue(&rig.graph, &issue, &flags, &mut rig.ctx()).expect_err(stage);
    assert!(
      refused.message.contains(recovery),
      "{stage}: {}",
      refused.message
    );
    assert_eq!(rig.leases(), 0);
  }
  rig.write_record(
    "comemory-255",
    &json!({"stage": "running", "kind": "claude", "launches": 1}),
  );
  flags.kind = Some("codex".into());
  let moved = launch_issue(&rig.graph, &issue, &flags, &mut rig.ctx()).expect_err("host change");
  assert_eq!(moved.message, "host change requires explicit --replace");
  for source in ["{", "[]", "null"] {
    std::fs::write(rig.state().join("issues/comemory-255.json"), source).expect("record");
    let malformed = launch_issue(&rig.graph, &issue, &flags, &mut rig.ctx()).expect_err(source);
    assert!(
      malformed.message.starts_with("invalid launch record "),
      "{}",
      malformed.message
    );
  }
  assert_eq!(rig.leases(), 0);
}

/// The refusal herdr 0.9.5 sent in #613 while the new pane's shell was starting.
fn busy() -> crate::herdr_state::Scripted {
  crate::herdr_state::Scripted::Error {
    code: "agent_pane_busy".into(),
    message: "agent target pane wE0:p1 is not an available shell".into(),
  }
}

/// A live forced launch of comemory-255 with GitHub answering the default branch.
fn launch_live(rig: &Rig) -> Result<serde_json::Value, crate::launch::Failure> {
  let mut flags = opts();
  flags.force = true;
  rig.launch(&flags)
}

fn starts(rig: &Rig) -> usize {
  rig
    .herdr
    .calls()
    .iter()
    .filter(|(method, _)| method == "agent.start")
    .count()
}

#[test]
fn a_busy_pane_is_retried_until_the_agent_starts() {
  let mut rig = Rig::new();
  rig.checkout("comemory-255");
  rig.claude_agents();
  rig.herdr.script("agent.start", busy());
  let result = launch_live(&rig).unwrap_or_else(|err| panic!("{}", err.message));
  let record = rig.record("comemory-255");
  assert_eq!(record["stage"], "running");
  assert_eq!(record["lifecycle_outcome"], "started");
  let retry = format!(
    "herdr agent.start: pane {} busy; retry in 250 ms",
    record["pane_id"].as_str().expect("pane")
  );
  assert!(
    result["commands"]
      .as_array()
      .is_some_and(|lines| lines.iter().any(|line| line == retry.as_str())),
    "{result}"
  );
  assert!(record.get("lifecycle_error").is_none(), "{record}");
  assert_eq!(starts(&rig), 2);
}

#[test]
fn a_pane_busy_past_the_budget_releases_the_launch() {
  let mut rig = Rig::new();
  rig.checkout("comemory-255");
  rig.claude_agents();
  for _ in 0..40 {
    rig.herdr.script("agent.start", busy());
  }
  let refused = launch_live(&rig).expect_err("busy past the budget");
  assert!(
    refused.message.contains("agent_pane_busy"),
    "{}",
    refused.message
  );
  let record = rig.record("comemory-255");
  assert!(record.get("stage").is_none(), "{record}");
  assert!(record.get("lease_token").is_none(), "{record}");
  assert_eq!(record["lifecycle_outcome"], "not-started");
  assert_eq!(record["native_error"]["code"], "agent_pane_busy");
  assert_eq!(record["native_error"]["uncertain"], false);
  assert_eq!(rig.leases(), 0);
  assert!(starts(&rig) > 1);
  rig.herdr.state().scripted.clear();
  let relaunched = launch_live(&rig);
  relaunched.unwrap_or_else(|err| panic!("{}", err.message));
  let record = rig.record("comemory-255");
  assert_eq!(record["stage"], "running");
  assert_eq!(rig.leases(), 1);
}

#[test]
fn a_busy_pane_whose_lease_cannot_be_released_stays_uncertain() {
  let mut rig = Rig::new();
  rig.checkout("comemory-255");
  rig.claude_agents();
  for _ in 0..40 {
    rig.herdr.script("agent.start", busy());
  }
  launch_live(&rig).expect_err("busy past the budget");
  let worktree = rig.record("comemory-255")["worktree"]
    .as_str()
    .expect("worktree")
    .to_owned();
  let job = LeaseRequest {
    kind: LeaseKind::Job,
    key: "comemory-255".into(),
    state_dir: rig.graph.state_dir.clone(),
    host: None,
    host_cap: None,
    epic_cap: None,
    worktree: Some(worktree),
  };
  let resources = rig.tmp.path().join("resources");
  acquire_lease(&resources, &job, SystemTime::now()).expect("job");
  let refused = launch_live(&rig).expect_err("still busy");
  assert!(refused.message.contains("agent_pane_busy"));
  let record = rig.record("comemory-255");
  assert_eq!(record["stage"], "uncertain");
  assert_eq!(
    record["lease_release_error"],
    "active jobs prevent agent lease release"
  );
  assert_eq!(record["lifecycle_outcome"], "not-started");
}
