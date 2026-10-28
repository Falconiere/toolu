use std::path::Path;

use toolu_runtime::env::Env;

use super::{State, Store, opencode_shim, version_of};
use crate::host::Target;

fn repo_plugin(name: &str) -> std::path::PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../../plugins")
    .join(name)
}

fn env(home: &Path) -> Env {
  Env::from_pairs([("HOME", home.to_str().unwrap())])
}

fn store(home: &Path) -> Store {
  Store::locate(&env(home), Target::Cursor)
}

#[test]
fn a_plugin_is_copied_marked_and_replaced_in_place() {
  let home = tempfile::tempdir().unwrap();
  let store = store(home.path());
  assert!(store.dir.ends_with(".cursor/plugins/local"));
  assert_eq!(store.state("ast-grep"), State::Absent);
  let source = repo_plugin("ast-grep");
  store.put(&source, "ast-grep", "v1.0.0").unwrap();
  let want = version_of(&source);
  assert!(want.is_some());
  assert_eq!(store.state("ast-grep"), State::Ours(want));
  assert!(store.path("ast-grep").join("hooks/hooks.json").is_file());
  store.put(&source, "ast-grep", "v1.0.1").unwrap();
  let marker = std::fs::read_to_string(store.path("ast-grep/.toolu-install.json")).unwrap();
  assert!(marker.contains("v1.0.1"), "{marker}");
  assert_eq!(store.ours(), ["ast-grep"]);
  assert!(!store.dir.join(".ast-grep.toolu-staging").exists());
}

#[test]
fn a_directory_without_the_marker_is_foreign_and_never_listed_as_ours() {
  let home = tempfile::tempdir().unwrap();
  let store = store(home.path());
  std::fs::create_dir_all(store.path("mine")).unwrap();
  assert_eq!(store.state("mine"), State::Foreign);
  assert_eq!(store.ours(), Vec::<String>::new());
}

#[test]
fn take_removes_a_copy_and_reports_a_missing_one() {
  let home = tempfile::tempdir().unwrap();
  let store = store(home.path());
  store
    .put(&repo_plugin("ast-grep"), "ast-grep", "v1")
    .unwrap();
  store.take("ast-grep").unwrap();
  assert_eq!(store.state("ast-grep"), State::Absent);
  assert!(
    store
      .take("ast-grep")
      .unwrap_err()
      .contains("cannot remove")
  );
}

#[test]
fn symlinks_are_not_followed_into_the_copy() {
  let home = tempfile::tempdir().unwrap();
  let source = home.path().join("src/p");
  std::fs::create_dir_all(source.join(".claude-plugin")).unwrap();
  std::fs::write(
    source.join(".claude-plugin/plugin.json"),
    r#"{"version":"1.2.3"}"#,
  )
  .unwrap();
  std::os::unix::fs::symlink("/etc", source.join("out")).unwrap();
  let store = store(home.path());
  store.put(&source, "p", "v1").unwrap();
  assert!(!store.path("p").join("out").exists());
  assert_eq!(store.state("p"), State::Ours(Some("1.2.3".to_owned())));
}

#[test]
fn an_opencode_copy_gets_the_shim_for_its_hooks_and_catalog() {
  let home = tempfile::tempdir().unwrap();
  let opencode = Store::locate(&env(home.path()), Target::Opencode);
  assert!(opencode.shim);
  assert!(opencode.dir.ends_with(".config/opencode/plugins"));
  opencode.put(&repo_plugin("toolu"), "toolu", "v1").unwrap();
  let shim = std::fs::read_to_string(opencode.path("toolu/server.js")).unwrap();
  assert!(shim.contains("const PLUGIN = \"toolu\";"), "{shim}");
  assert!(shim.contains("\"tool.execute.before\""));
  assert!(shim.contains("const CATALOG = true;"));
  assert!(matches!(opencode.state("toolu"), State::Ours(Some(_))));
  let cursor = store(home.path());
  cursor.put(&repo_plugin("toolu"), "toolu", "v1").unwrap();
  assert!(!cursor.path("toolu/server.js").exists());
}

#[test]
fn the_shim_of_a_plugin_without_hooks_registers_none() {
  let shim = opencode_shim(&repo_plugin("brainstorm"), "brainstorm").unwrap();
  assert!(shim.contains("const HOOKS = [];"), "{shim}");
  assert!(shim.contains("const CATALOG = true;"));
  let bare = tempfile::tempdir().unwrap();
  let none = opencode_shim(bare.path(), "bare").unwrap();
  assert!(none.contains("const CATALOG = false;"));
  std::fs::create_dir_all(bare.path().join("hooks")).unwrap();
  std::fs::write(bare.path().join("hooks/hooks.json"), "{").unwrap();
  assert!(
    opencode_shim(bare.path(), "bare")
      .unwrap_err()
      .contains("not JSON")
  );
}
