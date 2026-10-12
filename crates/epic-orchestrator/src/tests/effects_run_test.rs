use super::job;
use crate::executor::Call;
use crate::model::Action;
use crate::server::{Engine, Fault};

#[test]
fn a_checkpoint_without_a_worktree_applies_and_a_script_action_defers_without_scripts() {
  let fix = crate::source_fix::fixture(true).expect("fixture");
  let mut engine = Engine::open(fix.engine_paths, None, Fault::None).expect("open");
  if let Some(issue) = engine.world.issues.get_mut("a") {
    issue.worktree = None;
  }
  let checkpoint = Call {
    key: "a".into(),
    action: Action::Checkpoint,
    token: "t".into(),
  };
  assert_eq!(job(&engine, &checkpoint)(), "applied");
  let merge = Call {
    key: "a".into(),
    action: Action::Merge,
    token: "t".into(),
  };
  assert_eq!(job(&engine, &merge)(), "deferred");
}
