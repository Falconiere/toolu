use serde_json::json;

#[test]
fn hc_plan_names_both_remote_and_local_cleanup() {
  let record = json!({"backend":"herdr-app", "machine":"peer", "remote_worktree":"/remote/wt", "terminal_address":"peer/t1", "worktree":"/local/wt", "checkout":"/local/repo", "branch":"feature"});
  let actions = super::plan_actions(record.as_object().expect("record"), "k", false);
  assert!(
    actions
      .iter()
      .any(|line| line.contains("/remote/wt") && line.contains("peer"))
  );
  assert!(actions.iter().any(|line| line.contains("/local/wt")));
}
