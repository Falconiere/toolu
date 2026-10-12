use serde_json::json;
use toolu_runtime::env::Env;

use super::delete_branch;
use crate::finish_rig::{layout, patch, read};
use crate::support::git;

#[test]
fn a_branch_held_elsewhere_is_retried_until_it_is_gone() {
  let l = layout();
  let env = Env::from_pairs([("PATH", "/usr/bin:/bin"), ("HOME", "/tmp")]);
  let blocker = l.tmp.path().join("branch-owner");
  git(
    &l.main,
    &["worktree", "remove", "--force", &l.wt.display().to_string()],
  );
  git(
    &l.main,
    &[
      "worktree",
      "add",
      "-q",
      &blocker.display().to_string(),
      "feat/1-x",
    ],
  );
  let record = read(&l.record);
  let record = record.as_object().cloned().expect("object");
  assert_eq!(delete_branch(&record, true, false, &env), Ok(false));
  assert_eq!(delete_branch(&record, false, false, &env), Ok(false));
  assert_eq!(delete_branch(&record, true, true, &env), Ok(false));
  git(
    &l.main,
    &[
      "worktree",
      "remove",
      "--force",
      &blocker.display().to_string(),
    ],
  );
  assert_eq!(delete_branch(&record, true, false, &env), Ok(true));
  assert_eq!(
    delete_branch(&record, true, false, &env),
    Ok(true),
    "an absent branch is deleted"
  );
  patch(
    &l,
    &json!({"branch_deleted": true, "checkout": "/nonexistent"}),
  );
  let recorded = read(&l.record).as_object().cloned().expect("object");
  assert_eq!(delete_branch(&recorded, true, false, &env), Ok(true));
  let mut broken = recorded;
  broken.remove("branch_deleted");
  assert_eq!(
    delete_branch(&broken, true, false, &env),
    Err("cannot verify local branch refs/heads/feat/1-x".to_owned())
  );
}
