use super::*;
use crate::dispatch::sandbox::sandbox;

#[test]
fn unfinished_remote_cleanup_preserves_the_recorded_worktree() {
  let sb = sandbox();
  let mut dispatcher = Dispatcher::new(&sb.options(false)).unwrap();
  dispatcher.state["herdrWorktree"] =
    serde_json::json!({"remote": {"machine": "vm", "repo": "/repo", "run": "r"}});
  let error = dispatcher.remove_remote().unwrap_err();
  assert_eq!(error.code, "remote_preserved");
  assert!(dispatcher.remote().is_some());
}
