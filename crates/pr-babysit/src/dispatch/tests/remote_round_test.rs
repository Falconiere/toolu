use super::*;
use crate::dispatch::sandbox::sandbox;

#[test]
fn only_a_settled_round_without_pending_setup_starts_a_new_attempt() {
  let sb = sandbox();
  let mut dispatcher = Dispatcher::new(&sb.options(false)).unwrap();
  dispatcher.state["herdrWorktree"] = json!({"remote": {"setupPending": false}});
  dispatcher.state["fixer"] = json!({"status": "done", "groups": []});
  assert!(dispatcher.new_remote_attempt());
  dispatcher.state["herdrWorktree"]["remote"]["setupPending"] = json!(true);
  assert!(!dispatcher.new_remote_attempt());
  dispatcher.state["fixer"]["status"] = json!("running");
  assert!(!dispatcher.new_remote_attempt());
  dispatcher.state["herdrWorktree"]["remote"]["setupPending"] = json!(false);
  dispatcher.state["herdrWorktree"]["remote"]["lastFixer"] =
    json!({"status":"failed", "groups":[]});
  dispatcher.state["fixer"] = Value::Null;
  assert!(dispatcher.new_remote_attempt());
}

#[test]
fn a_settled_round_cannot_replace_a_live_or_unverifiable_fixer() {
  let sb = sandbox();
  let mut dispatcher = Dispatcher::new(&sb.options(false)).unwrap();
  dispatcher.state["fixer"] = json!({"status":"failed", "groups":[{"status":"running"}]});
  assert_eq!(
    dispatcher.require_prior_stopped().unwrap_err().code,
    "fixer_running"
  );
  dispatcher.state["fixer"]["groups"][0] =
    json!({"status":"failed", "hcAddress":"t1", "hcOwner":"toolu.owner=x", "hcSession":"other"});
  assert!(dispatcher.require_prior_stopped().is_err());
}
