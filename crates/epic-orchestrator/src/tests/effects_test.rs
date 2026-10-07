use super::invoke;
use crate::model::Action;

#[test]
fn a_missing_script_directory_defers_the_action() {
  assert_eq!(invoke(None, Action::Merge, "{}"), "deferred");
}
