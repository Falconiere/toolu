use serde_json::json;

use super::{leftovers, shell_pid, snapshot_ref};
use crate::launch::record::Record;
use crate::launch_rig::Rig;

#[test]
fn shell_pid_is_none_when_herdr_does_not_know_the_pane() {
  let rig = Rig::new();
  let mut record = Record::new();
  record.insert("pane_id".to_owned(), json!("w9:p9"));
  assert_eq!(shell_pid(&mut rig.ctx(), &record), Ok(None));
}

#[test]
fn leftovers_are_empty_when_the_worktree_is_gone() {
  let rig = Rig::new();
  assert_eq!(leftovers("/nonexistent-wt", &rig.ctx()), Ok(Vec::new()));
}

#[test]
fn snapshot_ref_fails_when_the_worktree_is_gone() {
  let refused = snapshot_ref("/nonexistent-wt", "k-1").expect_err("absent");
  assert!(
    refused.starts_with("required checkpoint failed"),
    "{refused}"
  );
}
