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

#[test]
fn hold_deferred_and_cleanup_become_judgments() {
  let mut world = World::new(0);
  arm(&mut world, Action::Launch, false);
  let _steps = commit_effect(&mut world, "a", "hold");
  assert!(
    world
      .attention
      .iter()
      .any(|item| item.kind == "coupling-hold")
  );
  arm(&mut world, Action::Launch, false);
  let _steps = commit_effect(&mut world, "a", "deferred");
  assert_eq!(
    world.issues.get("a").expect("a").deferred.as_deref(),
    Some("launch")
  );
  arm(&mut world, Action::Cleanup, false);
  let _steps = commit_effect(&mut world, "a", "weird");
  assert!(
    world
      .attention
      .iter()
      .any(|item| item.kind == "cleanup-incomplete")
  );
  arm(&mut world, Action::Launch, false);
  let _steps = commit_effect(&mut world, "a", "applied");
  assert_eq!(world.issues.get("a").expect("a").stage, "running");
}

#[test]
fn a_recovered_effect_reconciles_or_defers() {
  let mut world = World::new(0);
  arm(&mut world, Action::Merge, true);
  let _steps = commit_effect(&mut world, "a", "deferred");
  assert!(world.issues.get("a").expect("a").pending.is_none());
  arm(&mut world, Action::Merge, true);
  let _steps = commit_effect(&mut world, "a", "merged");
  let pending = world
    .issues
    .get("a")
    .expect("a")
    .pending
    .clone()
    .expect("pending");
  assert!(!pending.recovered);
  assert_eq!(pending.phase, 2);
  arm(&mut world, Action::Merge, true);
  let _steps = commit_effect(&mut world, "a", "open");
  assert!(
    !world
      .issues
      .get("a")
      .expect("a")
      .pending
      .as_ref()
      .expect("open")
      .recovered
  );
  arm(&mut world, Action::Merge, true);
  let _steps = commit_effect(&mut world, "a", "nope");
  assert!(world.attention.iter().any(|item| item.kind == "failed"));
  let _steps = commit_effect(&mut world, "missing", "applied");
}

fn arm(world: &mut World, action: Action, recovered: bool) {
  let issue = ensure_issue(world, "a", "one", "/tmp");
  let mut pending = fresh(action, "t".to_owned());
  pending.recovered = recovered;
  pending.phase = 1;
  issue.pending = Some(pending);
  issue.stage = "running".to_owned();
}
