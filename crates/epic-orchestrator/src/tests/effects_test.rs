use super::invoke;
use crate::model::Action;

#[test]
fn a_missing_script_directory_defers_the_action() {
  assert_eq!(invoke(None, Action::Merge, "{}"), "deferred");
}

#[test]
fn a_script_outcome_is_the_printed_field() {
  let dir = tempfile::tempdir().expect("temp");
  assert_eq!(invoke(Some(dir.path()), Action::Merge, "{}"), "failed");
  let merge = dir.path().join("merge");
  std::fs::write(&merge, "echo '{\"outcome\":\"applied\"}'\n").expect("script");
  assert_eq!(invoke(Some(dir.path()), Action::Merge, "{}"), "applied");
  std::fs::write(&merge, "echo not-json\n").expect("bad");
  assert_eq!(invoke(Some(dir.path()), Action::Merge, "{}"), "failed");
  std::fs::write(
    dir.path().join("reconcile"),
    "echo '{\"state\":\"open\"}'\n",
  )
  .expect("reconcile");
  assert_eq!(invoke(Some(dir.path()), Action::Reconcile, "{}"), "open");
  std::fs::write(&merge, "exit 1\n").expect("fail");
  assert_eq!(invoke(Some(dir.path()), Action::Merge, "{}"), "failed");
}
