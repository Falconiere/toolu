use super::commit_effect;
use crate::logic::ensure_issue;
use crate::model::{Action, World, fresh};

#[test]
fn an_applied_merge_moves_to_cleaning() {
  let mut world = World::new(0);
  {
    let issue = ensure_issue(&mut world, "a", "one", "/tmp");
    issue.pending = Some(fresh(Action::Merge, "t1".to_owned()));
    "running".clone_into(&mut issue.stage);
  }
  let _steps = commit_effect(&mut world, "a", "applied");
  assert_eq!(world.issues.get("a").expect("a").stage, "cleaning");
}
