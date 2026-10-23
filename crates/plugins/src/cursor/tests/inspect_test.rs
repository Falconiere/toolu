use super::{list, remove};
use crate::cursor::Cursor;
use crate::cursor::change::install;
use crate::cursor::store::State;
use crate::cursor::tests::context;
use crate::step::Status;
use crate::support::{Sandbox, catalog, names};

#[test]
fn list_marks_what_toolu_installed_with_its_version() {
  let sb = Sandbox::new();
  let origin = sb.cursor_origin("v9.9.8", None);
  let cursor = Cursor::new(&context(&sb, "9.9.8", &origin)).unwrap();
  install(&cursor, &catalog(), &names(&["toolu"]), false).unwrap();
  std::fs::create_dir_all(cursor.store.path("ast-grep")).unwrap();
  let rows = list(&cursor, &catalog());
  let toolu = rows.iter().find(|row| row.name == "toolu").unwrap();
  assert!(toolu.installed && toolu.enabled && toolu.version.is_some());
  let foreign = rows.iter().find(|row| row.name == "ast-grep").unwrap();
  assert!(!foreign.installed && foreign.version.is_none());
  assert_eq!(rows.len(), catalog().plugins.len());
}

#[test]
fn remove_deletes_only_toolus_own_copies() {
  let sb = Sandbox::new();
  let origin = sb.cursor_origin("v9.9.8", None);
  let cursor = Cursor::new(&context(&sb, "9.9.8", &origin)).unwrap();
  install(&cursor, &catalog(), &names(&["toolu"]), false).unwrap();
  std::fs::create_dir_all(cursor.store.path("ast-grep")).unwrap();
  let dry = remove(&cursor, &names(&["toolu"]), true);
  assert_eq!(dry[0].status, Status::Skipped);
  assert!(matches!(cursor.store.state("toolu"), State::Ours(_)));
  let steps = remove(&cursor, &names(&["toolu", "ast-grep", "jev"]), false);
  let got: Vec<Status> = steps.iter().map(|step| step.status).collect();
  assert_eq!(got, [Status::Removed, Status::Skew, Status::Removed]);
  assert_eq!(cursor.store.state("toolu"), State::Absent);
  assert_eq!(cursor.store.state("ast-grep"), State::Foreign);
}
