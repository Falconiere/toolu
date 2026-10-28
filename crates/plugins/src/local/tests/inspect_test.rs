use super::{list, remove};
use crate::host::Target;
use crate::local::Local;
use crate::local::change::install;
use crate::local::store::State;
use crate::local::tests::context;
use crate::step::Status;
use crate::support::{Sandbox, catalog, names};

#[test]
fn list_marks_what_toolu_installed_with_its_version() {
  let sb = Sandbox::new();
  let origin = sb.local_origin("v9.9.8", None);
  let local = Local::new(&context(&sb, "9.9.8", &origin), Target::Cursor).unwrap();
  install(&local, &catalog(), &names(&["toolu"]), false).unwrap();
  std::fs::create_dir_all(local.store.path("ast-grep")).unwrap();
  let rows = list(&local, &catalog());
  let toolu = rows.iter().find(|row| row.name == "toolu").unwrap();
  assert!(toolu.installed && toolu.enabled && toolu.version.is_some());
  let foreign = rows.iter().find(|row| row.name == "ast-grep").unwrap();
  assert!(!foreign.installed && foreign.version.is_none());
  assert_eq!(rows.len(), catalog().plugins.len());
}

#[test]
fn remove_deletes_only_toolus_own_copies() {
  let sb = Sandbox::new();
  let origin = sb.local_origin("v9.9.8", None);
  let local = Local::new(&context(&sb, "9.9.8", &origin), Target::Cursor).unwrap();
  install(&local, &catalog(), &names(&["toolu"]), false).unwrap();
  std::fs::create_dir_all(local.store.path("ast-grep")).unwrap();
  let dry = remove(&local, &names(&["toolu"]), true);
  assert_eq!(dry[0].status, Status::Skipped);
  assert!(matches!(local.store.state("toolu"), State::Ours(_)));
  let steps = remove(&local, &names(&["toolu", "ast-grep", "jev"]), false);
  let got: Vec<Status> = steps.iter().map(|step| step.status).collect();
  assert_eq!(got, [Status::Removed, Status::Skew, Status::Removed]);
  assert_eq!(local.store.state("toolu"), State::Absent);
  assert_eq!(local.store.state("ast-grep"), State::Foreign);
}
