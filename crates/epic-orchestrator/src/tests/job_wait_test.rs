use toolu_protocol::exit::Exit;
use toolu_runtime::env::Env;

use super::run;

#[test]
fn wait_outside_a_git_worktree_fails_without_touching_results() {
  let dir = tempfile::tempdir().unwrap();
  let outcome = run(&Env::process(), dir.path(), "", 0);
  assert_eq!(outcome.exit, Exit::Failure);
  assert!(
    outcome
      .stderr
      .unwrap_or_default()
      .contains("no Git worktree")
  );
}
