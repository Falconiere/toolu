use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

fn touch(path: &Path, time: SystemTime) {
  let file = std::fs::OpenOptions::new().write(true).open(path).unwrap();
  file.set_modified(time).unwrap();
}

fn write(dir: &Path, name: &str, text: &str, time: SystemTime) -> PathBuf {
  let path = dir.join(name);
  if let Some(parent) = path.parent() {
    std::fs::create_dir_all(parent).unwrap();
  }
  std::fs::write(&path, text).unwrap();
  touch(&path, time);
  path
}

fn read(path: &Path) -> Vec<u8> {
  std::fs::read(path).unwrap()
}

/// `cache/toolu/toolu/<version>/` with one settings file and a plugin manifest.
fn version(root: &Path, version: &str, file: &str, text: &str, time: SystemTime) -> PathBuf {
  let plugin = root.join("toolu").join(version);
  write(&plugin, ".claude-plugin/plugin.json", "{}\n", time);
  write(&plugin, &format!("settings/{file}"), text, time);
  plugin
}

fn epoch() -> SystemTime {
  SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000)
}

/// 9.0.0 and an edited 10.1.1 beside a shipped 10.2.0. `outside` is the symlink target.
fn edited_cache(dir: &Path) -> (PathBuf, PathBuf) {
  let installed = epoch();
  let edited = installed + Duration::from_secs(120);
  let root = dir.join("cache/toolu");
  version(&root, "9.0.0", "bash-denylist.txt", "from-nine\n", edited);
  let previous = version(&root, "10.1.1", "bash-denylist.txt", "user deny\n", edited);
  touch(&previous.join(".claude-plugin/plugin.json"), installed);
  write(
    &previous,
    "settings/commit-prefixes.txt",
    "old prefixes\n",
    installed,
  );
  write(
    &previous,
    "settings/recent.txt",
    "old recent\n",
    installed + Duration::from_secs(1),
  );
  write(&previous, "settings/mine.txt", "added\n", installed);
  let outside = dir.join("secret");
  std::fs::write(&outside, "nope\n").unwrap();
  std::os::unix::fs::symlink(&outside, previous.join("settings/linked.txt")).unwrap();
  let current = version(
    &root,
    "10.2.0",
    "bash-denylist.txt",
    "shipped deny\n",
    installed,
  );
  write(
    &current,
    "settings/commit-prefixes.txt",
    "new prefixes\n",
    installed,
  );
  write(&current, "settings/recent.txt", "new recent\n", installed);
  (current, outside)
}

#[test]
fn edited_settings_move_forward_and_unmodified_files_stay_on_the_new_release() {
  let dir = tempfile::tempdir().unwrap();
  let (current, outside) = edited_cache(dir.path());
  let done = super::migrate_plugin_settings(&current).unwrap();
  assert_eq!(done.from.as_deref(), Some("10.1.1"));
  assert_eq!(done.kept, ["bash-denylist.txt", "mine.txt"]);
  assert_eq!(
    read(&current.join("settings/bash-denylist.txt")),
    b"user deny\n"
  );
  assert_eq!(
    read(&current.join("settings/commit-prefixes.txt")),
    b"new prefixes\n"
  );
  assert_eq!(read(&current.join("settings/recent.txt")), b"new recent\n");
  assert_eq!(read(&current.join("settings/mine.txt")), b"added\n");
  assert!(!current.join("settings/linked.txt").exists());
  assert_eq!(read(&outside), b"nope\n");
  assert_eq!(
    std::fs::read_to_string(current.join("settings/.migrated-from")).unwrap(),
    "10.1.1\n"
  );
  std::fs::write(
    current.join("settings/bash-denylist.txt"),
    "changed-after\n",
  )
  .unwrap();
  assert_eq!(
    super::migrate_plugin_settings(&current).unwrap().kept,
    Vec::<String>::new()
  );
  assert_eq!(
    read(&current.join("settings/bash-denylist.txt")),
    b"changed-after\n"
  );
}

#[test]
fn an_edit_already_in_the_new_version_is_kept() {
  let dir = tempfile::tempdir().unwrap();
  let installed = epoch();
  let edited = installed + Duration::from_secs(120);
  let root = dir.path().join("cache/toolu");
  let previous = version(&root, "10.1.1", "bash-denylist.txt", "old edit\n", edited);
  touch(&previous.join(".claude-plugin/plugin.json"), installed);
  let current = version(&root, "10.2.0", "bash-denylist.txt", "new edit\n", edited);
  touch(&current.join(".claude-plugin/plugin.json"), installed);
  let done = super::migrate_plugin_settings(&current).unwrap();
  assert_eq!(done.kept, Vec::<String>::new());
  assert_eq!(
    read(&current.join("settings/bash-denylist.txt")),
    b"new edit\n"
  );
}

#[test]
fn a_version_with_no_older_sibling_records_that_nothing_was_migrated() {
  let dir = tempfile::tempdir().unwrap();
  let current = version(
    &dir.path().join("cache/toolu"),
    "10.2.0",
    "bash-denylist.txt",
    "shipped\n",
    epoch(),
  );
  let done = super::migrate_plugin_settings(&current).unwrap();
  assert_eq!(done.kept, Vec::<String>::new());
  assert_eq!(
    read(&current.join("settings/bash-denylist.txt")),
    b"shipped\n"
  );
  assert_eq!(
    std::fs::read_to_string(current.join("settings/.migrated-from")).unwrap(),
    "none\n"
  );
}

#[test]
fn a_plugin_root_outside_a_version_directory_is_left_alone() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path().join("plugins/local/toolu");
  std::fs::create_dir_all(root.join("settings")).unwrap();
  std::fs::write(root.join("settings/bash-denylist.txt"), "local\n").unwrap();
  let done = super::migrate_plugin_settings(&root).unwrap();
  assert_eq!(
    done,
    super::SettingsMigration {
      from: None,
      kept: Vec::new()
    }
  );
  assert_eq!(read(&root.join("settings/bash-denylist.txt")), b"local\n");
  assert!(!root.join("settings/.migrated-from").exists());
}
