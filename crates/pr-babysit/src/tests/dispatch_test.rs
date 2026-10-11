use serde_json::json;

use super::*;
use crate::dispatch::sandbox::sandbox;

#[test]
fn wait_refuses_a_dry_run_and_a_malformed_timeout() {
  let sb = sandbox();
  let dry = dispatch_fix(&Verb::Wait(None), &sb.options(true)).unwrap_err();
  assert_eq!(
    (dry.code, dry.message.as_str()),
    ("usage", "--dry-run applies to start and cleanup, not wait")
  );
  for bad in ["1.5", "-1", "soon"] {
    let failure = dispatch_fix(&Verb::Wait(Some(bad)), &sb.options(false)).unwrap_err();
    assert_eq!(failure.message, "--timeout-seconds must be a whole number");
  }
}

#[test]
fn each_verb_runs_against_the_slot_state() {
  let sb = sandbox();
  let none = dispatch_fix(&Verb::Wait(Some("1")), &sb.options(false)).unwrap();
  assert_eq!(none, json!({"version": 1, "status": "none"}));
  let (plan, items) = sb.plan("opencode", None);
  let started = dispatch_fix(&Verb::Start(sb.start(&plan, &items)), &sb.options(false)).unwrap();
  assert_eq!(started["reason"], "agent_start_failed");
  let cleaned = dispatch_fix(&Verb::Cleanup, &sb.options(false)).unwrap();
  assert_eq!(cleaned["status"], "cleaned");
  let missing = sb.dir.path().join("missing.json");
  let options = dispatcher::Options {
    state_file: &missing,
    ..sb.options(false)
  };
  assert!(dispatch_fix(&Verb::Cleanup, &options).is_err());
}
