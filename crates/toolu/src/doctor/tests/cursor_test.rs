use std::path::{Path, PathBuf};

use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;

use super::super::checks::Status;
use super::super::inventory::plugins_check;
use super::collect;

fn repo_plugin(name: &str) -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../../plugins")
    .join(name)
}

fn copy(from: &Path, to: &Path) {
  std::fs::create_dir_all(to).unwrap();
  for entry in std::fs::read_dir(from).unwrap() {
    let entry = entry.unwrap();
    let target = to.join(entry.file_name());
    if entry.path().is_dir() {
      copy(&entry.path(), &target);
    } else {
      std::fs::copy(entry.path(), target).unwrap();
    }
  }
}

fn roots(home: &Path) -> Roots {
  Roots::new(
    Env::from_pairs([("HOME", home.to_str().unwrap())]),
    Some(Host::Cursor),
  )
}

#[test]
fn an_empty_cursor_reports_no_plugins_without_the_unknown_warning() {
  let home = tempfile::tempdir().unwrap();
  let check = plugins_check(&collect(&roots(home.path())));
  assert_eq!(check.status, Status::Warn);
  assert!(
    check.summary.contains("no toolu plugin"),
    "{}",
    check.summary
  );
  assert!(!check.summary.contains("unknown"));
}

#[test]
fn local_and_cached_plugins_are_listed_with_their_versions() {
  let home = tempfile::tempdir().unwrap();
  let cursor = home.path().join(".cursor/plugins");
  copy(&repo_plugin("toolu"), &cursor.join("local/toolu"));
  std::fs::write(cursor.join("local/toolu/.toolu-install.json"), "{}").unwrap();
  let cache = cursor.join("cache/toolu/ast-grep/abc123");
  copy(&repo_plugin("ast-grep"), &cache);
  std::fs::write(cache.join(".cache-complete"), "").unwrap();
  let inventory = collect(&roots(home.path()));
  let names: Vec<&str> = inventory
    .plugins
    .iter()
    .map(|plugin| plugin.name.as_str())
    .collect();
  assert_eq!(names, ["ast-grep", "toolu"]);
  assert!(
    inventory
      .plugins
      .iter()
      .all(|plugin| plugin.version.is_some())
  );
  assert_eq!(inventory.warning, None);
  let check = plugins_check(&inventory);
  assert_eq!(check.status, Status::Ok);
  assert_eq!(check.summary, "2 toolu plugins");
}

#[test]
fn an_incomplete_cache_and_an_unmarked_local_directory_do_not_count() {
  let home = tempfile::tempdir().unwrap();
  let cursor = home.path().join(".cursor/plugins");
  copy(
    &repo_plugin("ast-grep"),
    &cursor.join("cache/toolu/ast-grep/abc123"),
  );
  copy(&repo_plugin("toolu"), &cursor.join("local/toolu"));
  assert!(collect(&roots(home.path())).plugins.is_empty());
}

#[test]
fn a_plugin_without_hooks_json_warns() {
  let home = tempfile::tempdir().unwrap();
  let local = home.path().join(".cursor/plugins/local/toolu");
  copy(&repo_plugin("toolu"), &local);
  std::fs::write(local.join(".toolu-install.json"), "{}").unwrap();
  std::fs::remove_file(local.join("hooks/hooks.json")).unwrap();
  let inventory = collect(&roots(home.path()));
  assert_eq!(inventory.plugins.len(), 1);
  let warning = inventory.warning.unwrap();
  assert!(
    warning.contains("toolu has no hooks/hooks.json"),
    "{warning}"
  );
}
