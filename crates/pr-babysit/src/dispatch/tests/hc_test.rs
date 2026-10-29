use super::*;
use crate::dispatch::sandbox::sandbox;

#[test]
fn backend_is_pinned_and_local_worktree_is_reused() {
  let sb = sandbox();
  let env = sb.env.clone().with("TOOLU_EPIC_MULTIPLEXER", "herdr-app");
  let mut options = sb.options(false);
  options.env = &env;
  let mut dispatcher = Dispatcher::new(&options).unwrap();
  assert!(dispatcher.hc_backend());
  let created = dispatcher.worktree(&sb.root, "feat/fix").unwrap();
  assert_eq!(created["multiplexer"], "herdr-app");
  assert!(std::path::Path::new(created["path"].as_str().unwrap()).exists());
  let other = sb.env.clone().with("TOOLU_EPIC_MULTIPLEXER", "herdr");
  options.env = &other;
  let mut resumed = Dispatcher::new(&options).unwrap();
  assert!(resumed.hc_backend());
  assert_eq!(resumed.worktree(&sb.root, "feat/fix").unwrap(), created);
}

#[test]
fn legacy_worktree_stays_on_herdr_and_auto_detects_hc() {
  let sb = sandbox();
  let env = sb.env.clone().with("HC_TERMINAL_ID", "t1");
  let mut options = sb.options(false);
  options.env = &env;
  let mut dispatcher = Dispatcher::new(&options).unwrap();
  assert!(dispatcher.hc_backend());
  dispatcher.state["herdrWorktree"] = json!({"path": "/legacy"});
  assert!(!dispatcher.hc_backend());
}

#[test]
fn saved_terminal_is_not_reused_after_session_changes() {
  let sb = sandbox();
  let env = sb.env.clone().with("HC_SESSION", "new-session");
  let mut options = sb.options(false);
  options.env = &env;
  let dispatcher = Dispatcher::new(&options).unwrap();
  let group = json!({"hcAddress":"t1", "hcOwner":"toolu.owner=old", "hcSession":"old-session"});
  assert!(
    dispatcher
      .hc_terminal(&group)
      .unwrap_err()
      .message
      .contains("session differs")
  );
}

#[test]
fn recovered_remote_name_becomes_a_qualified_immutable_address() {
  let terminal = json!({"address":"t7"});
  assert_eq!(
    immutable_address(&terminal, None, Some("vm")).unwrap(),
    "vm/t7"
  );
  assert!(immutable_address(&terminal, Some("vm/t8"), Some("vm")).is_err());
  assert!(immutable_address(&json!({"address":"other/t7"}), None, Some("vm")).is_err());
}
