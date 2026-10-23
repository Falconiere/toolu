use std::path::Path;

use toolu_runtime::env::Env;

use super::{State, Store, version_of};

fn repo_plugin(name: &str) -> std::path::PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../../plugins")
    .join(name)
}

fn store(home: &Path) -> Store {
  Store::locate(&Env::from_pairs([("HOME", home.to_str().unwrap())]))
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
  assert!(store.ours().is_empty());
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
