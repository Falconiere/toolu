use std::ffi::OsStr;
use std::process::Command;

use super::{inherited, scrub, selects_repository};

#[test]
fn only_the_repository_selecting_variables_are_named() {
  for key in [
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_COMMON_DIR",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
  ] {
    assert!(selects_repository(OsStr::new(key)), "{key}");
  }
  assert!(!selects_repository(OsStr::new("GIT_CONFIG_NOSYSTEM")));
  assert!(!selects_repository(OsStr::new("PATH")));
}

#[test]
fn a_scrubbed_git_does_not_see_an_inherited_git_dir() {
  let dir = tempfile::tempdir().unwrap();
  let init = Command::new("git")
    .args(["init", "-q"])
    .arg(dir.path())
    .output()
    .unwrap();
  assert!(init.status.success());
  let mut command = Command::new("git");
  command
    .args(["rev-parse", "--git-dir"])
    .current_dir(dir.path())
    .env("GIT_DIR", "/no/such/git-dir");
  assert!(!command.output().unwrap().status.success());
  scrub(&mut command);
  let output = command.output().unwrap();
  assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), ".git");
}

#[test]
fn the_inherited_environment_keeps_the_path_and_drops_host_git_and_managed_keys() {
  let all = inherited(&[]);
  let keys: Vec<_> = all.iter().map(|(key, _)| key.to_string_lossy()).collect();
  assert!(keys.iter().any(|key| key == "PATH"), "{keys:?}");
  assert!(keys.iter().all(|key| !key.starts_with("TOOLU_")
    && !key.starts_with("CLAUDE_")
    && !key.starts_with("GIT_DIR")));
  let without = inherited(&["PATH"]);
  assert!(without.iter().all(|(key, _)| key != "PATH"));
}
