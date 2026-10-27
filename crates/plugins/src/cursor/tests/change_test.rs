use super::{install, update};
use crate::cursor::Cursor;
use crate::cursor::store::State;
use crate::cursor::tests::context;
use crate::step::Status;
use crate::support::{Sandbox, catalog, names};

fn statuses(steps: &[crate::step::Step]) -> Vec<(&str, Status)> {
  steps
    .iter()
    .map(|step| (step.name.as_str(), step.status))
    .collect()
}

#[test]
fn install_copies_dependencies_first_and_a_second_run_is_already() {
  let sb = Sandbox::new();
  let origin = sb.cursor_origin("v9.9.8", None);
  let cursor = Cursor::new(&context(&sb, "9.9.8", &origin)).unwrap();
  let steps = install(&cursor, &catalog(), &names(&["toolu", "ast-grep"]), false).unwrap();
  assert_eq!(
    statuses(&steps),
    [
      ("toolu", Status::Installed),
      ("ast-grep", Status::Installed)
    ]
  );
  assert!(matches!(cursor.store.state("toolu"), State::Ours(Some(_))));
  assert!(cursor.store.path("ast-grep/hooks/hooks.json").is_file());
  let again = install(&cursor, &catalog(), &names(&["toolu", "ast-grep"]), false).unwrap();
  assert_eq!(
    statuses(&again),
    [("toolu", Status::Already), ("ast-grep", Status::Already)]
  );
}

#[test]
fn a_dry_run_clones_nothing_and_copies_nothing() {
  let sb = Sandbox::new();
  let cursor = Cursor::new(&context(&sb, "9.9.8", "file:///nowhere")).unwrap();
  let steps = install(&cursor, &catalog(), &names(&["toolu"]), true).unwrap();
  assert_eq!(statuses(&steps), [("toolu", Status::Skipped)]);
  assert!(!cursor.checkout.dir.exists() && !cursor.store.dir.exists());
}

#[test]
fn an_unmarked_directory_is_left_untouched() {
  let sb = Sandbox::new();
  let origin = sb.cursor_origin("v9.9.8", None);
  let cursor = Cursor::new(&context(&sb, "9.9.8", &origin)).unwrap();
  std::fs::create_dir_all(cursor.store.path("toolu")).unwrap();
  std::fs::write(cursor.store.path("toolu/mine.txt"), "mine").unwrap();
  let steps = install(&cursor, &catalog(), &names(&["toolu"]), false).unwrap();
  assert_eq!(statuses(&steps), [("toolu", Status::Skew)]);
  assert!(cursor.store.path("toolu/mine.txt").is_file());
}

#[test]
fn a_missing_tag_stops_the_install_before_any_copy() {
  let sb = Sandbox::new();
  let origin = sb.cursor_origin("v9.9.8", None);
  let cursor = Cursor::new(&context(&sb, "0.0.1", &origin)).unwrap();
  assert!(install(&cursor, &catalog(), &names(&["toolu"]), false).is_err());
  assert!(!cursor.store.dir.exists());
}

#[test]
fn update_moves_to_a_newer_tag_and_reports_current_after() {
  let sb = Sandbox::new();
  let first = sb.cursor_origin("v9.9.8", None);
  let old = Cursor::new(&context(&sb, "9.9.8", &first)).unwrap();
  install(&old, &catalog(), &names(&["toolu"]), false).unwrap();
  let origin = sb.cursor_origin("v9.9.9", Some("9.9.9"));
  let new = Cursor::new(&context(&sb, "9.9.9", &origin)).unwrap();
  let steps = update(&new, &catalog(), &[], false).unwrap();
  assert_eq!(statuses(&steps), [("toolu", Status::Updated)]);
  assert_eq!(
    new.store.state("toolu"),
    State::Ours(Some("9.9.9".to_owned()))
  );
  let steps = update(&new, &catalog(), &[], false).unwrap();
  assert_eq!(statuses(&steps), [("toolu", Status::Current)]);
}

#[test]
fn update_with_nothing_installed_is_an_error() {
  let sb = Sandbox::new();
  let origin = sb.cursor_origin("v9.9.8", None);
  let cursor = Cursor::new(&context(&sb, "9.9.8", &origin)).unwrap();
  let err = update(&cursor, &catalog(), &[], false).unwrap_err();
  assert!(err.message.contains("nothing to update"), "{}", err.message);
}

#[test]
fn a_retired_plugin_toolu_installed_gets_a_remove_hint_instead_of_a_copy_failure() {
  let sb = Sandbox::new();
  let origin = sb.cursor_origin("v9.9.8", None);
  let cursor = Cursor::new(&context(&sb, "9.9.8", &origin)).unwrap();
  install(&cursor, &catalog(), &names(&["toolu", "ast-grep"]), false).unwrap();
  std::fs::rename(cursor.store.path("ast-grep"), cursor.store.path("jira")).unwrap();
  let steps = update(&cursor, &catalog(), &[], false).unwrap();
  let jira = steps.iter().find(|step| step.name == "jira");
  assert_eq!(
    jira.map(|step| (step.status, step.detail.as_str())),
    Some((
      Status::Skew,
      "retired from the toolu marketplace; remove it with `toolu plugins remove jira --host cursor --yes`"
    ))
  );
}
