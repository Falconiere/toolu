use std::path::Path;

use super::super::checks::Status;
use super::super::inventory::{Inventory, plugins_check};
use super::collect;

/// The inventory of an `OpenCode` whose config lives under `home`.
fn inventory(home: &Path) -> Inventory {
  let env = toolu_runtime::env::Env::from_pairs([("HOME", home.display().to_string())]);
  let host = Some(toolu_protocol::host::Host::Opencode);
  collect(&toolu_runtime::host::roots::Roots::new(env, host))
}

fn plugin(home: &Path, name: &str, marker: bool, shim: bool) {
  let dir = home.join(".config/opencode/plugins").join(name);
  std::fs::create_dir_all(dir.join(".claude-plugin")).unwrap();
  std::fs::write(
    dir.join(".claude-plugin/plugin.json"),
    r#"{"version":"8.6.0"}"#,
  )
  .unwrap();
  if marker {
    std::fs::write(dir.join(".toolu-install.json"), "{}").unwrap();
  }
  if shim {
    std::fs::write(dir.join("server.js"), "export default {}").unwrap();
  }
}

#[test]
fn an_empty_opencode_reports_no_plugins() {
  let home = tempfile::tempdir().unwrap();
  let inventory = inventory(home.path());
  assert!(inventory.plugins.is_empty() && inventory.warning.is_none());
  let check = plugins_check(&inventory);
  assert_eq!(check.status, Status::Warn);
  assert!(
    check.summary.contains("no toolu plugin"),
    "{}",
    check.summary
  );
}

#[test]
fn only_marked_directories_count_with_their_versions() {
  let home = tempfile::tempdir().unwrap();
  plugin(home.path(), "toolu", true, true);
  plugin(home.path(), "mine", false, true);
  let inventory = inventory(home.path());
  let names: Vec<&str> = inventory.plugins.iter().map(|p| p.name.as_str()).collect();
  assert_eq!(names, ["toolu"]);
  assert_eq!(inventory.plugins[0].version.as_deref(), Some("8.6.0"));
  assert!(inventory.warning.is_none());
}

#[test]
fn a_copy_without_its_shim_is_named_in_a_warning() {
  let home = tempfile::tempdir().unwrap();
  plugin(home.path(), "jev", true, false);
  let inventory = inventory(home.path());
  let warning = inventory.warning.unwrap();
  assert!(warning.contains("jev without server.js"), "{warning}");
  assert!(warning.contains("--host opencode"));
}

#[test]
fn a_config_that_still_names_the_deprecated_package_is_warned_about() {
  let home = tempfile::tempdir().unwrap();
  let config = home.path().join(".config/opencode");
  std::fs::create_dir_all(config.join("toolu")).unwrap();
  std::fs::write(
    config.join("toolu/package.json"),
    r#"{"dependencies":{"@toolu/opencode":"7.9.0"}}"#,
  )
  .unwrap();
  std::fs::write(config.join("opencode.json"), r#"{"plugins":["./toolu"]}"#).unwrap();
  let warning = inventory(home.path()).warning.unwrap();
  assert!(warning.contains("deprecated @toolu/opencode"), "{warning}");
  assert!(warning.contains("toolu/package.json"), "{warning}");
  assert!(!warning.contains("opencode.json,"), "{warning}");
  plugin(home.path(), "jev", true, false);
  let both = inventory(home.path()).warning.unwrap();
  assert!(
    both.contains("without server.js; ") && both.contains("@toolu/opencode"),
    "{both}"
  );
}

#[test]
fn a_plugins_path_that_is_not_a_directory_is_warned_about() {
  let home = tempfile::tempdir().unwrap();
  let config = home.path().join(".config/opencode");
  std::fs::create_dir_all(&config).unwrap();
  std::fs::write(config.join("plugins"), "not a directory").unwrap();
  let warning = inventory(home.path()).warning.unwrap();
  assert!(warning.contains("cannot read"), "{warning}");
}
