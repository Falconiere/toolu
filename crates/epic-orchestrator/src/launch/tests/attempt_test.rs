use serde_json::json;

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
