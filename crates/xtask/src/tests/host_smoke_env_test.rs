use std::ffi::OsString;
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;
use std::process::Command;

use super::{ChildEnv, find_tool, path_without_bun};

fn script(dir: &Path, name: &str, mode: u32) {
  std::fs::create_dir_all(dir).unwrap();
  let file = dir.join(name);
  std::fs::write(&file, "#!/bin/sh\n").unwrap();
  std::fs::set_permissions(&file, std::fs::Permissions::from_mode(mode)).unwrap();
}

fn join(dirs: &[&Path]) -> OsString {
  std::env::join_paths(dirs).unwrap()
}

#[test]
fn directories_with_an_executable_bun_leave_the_path() {
  let dir = tempfile::tempdir().unwrap();
  let with_bun = dir.path().join("with-bun");
  let plain = dir.path().join("plain");
  let inert = dir.path().join("inert");
  script(&with_bun, "bun", 0o755);
  script(&with_bun, "git", 0o755);
  script(&plain, "git", 0o755);
  script(&inert, "bun", 0o644);
  let filtered = path_without_bun(&join(&[&with_bun, &plain, &inert])).unwrap();
  let kept: Vec<_> = std::env::split_paths(&filtered).collect();
  assert_eq!(kept, vec![plain, inert]);
}

#[test]
fn an_empty_path_stays_empty() {
  assert_eq!(path_without_bun(&OsString::new()).unwrap(), OsString::new());
}

#[test]
fn find_tool_returns_the_first_executable() {
  let dir = tempfile::tempdir().unwrap();
  let first = dir.path().join("first");
  let second = dir.path().join("second");
  script(&first, "claude", 0o644);
  script(&second, "claude", 0o755);
  let found = find_tool("claude", &join(&[&first, &second])).unwrap();
  assert_eq!(found, second.join("claude"));
  assert!(find_tool("codex", &join(&[&first, &second])).is_none());
}

#[test]
fn tool_searches_the_unfiltered_path() {
  let dir = tempfile::tempdir().unwrap();
  let bin_dir = dir.path().join("shared");
  script(&bin_dir, "bun", 0o755);
  script(&bin_dir, "claude", 0o755);
  let env = ChildEnv::new(Path::new("/x/toolu"), &join(&[&bin_dir])).unwrap();
  assert_eq!(env.tool("claude").unwrap(), bin_dir.join("claude"));
  assert!(env.tool("codex").unwrap_err().contains("codex"));
}

#[test]
fn apply_sets_the_native_binary_and_drops_the_bun_overrides() {
  let dir = tempfile::tempdir().unwrap();
  let with_bun = dir.path().join("with-bun");
  let plain = dir.path().join("plain");
  script(&with_bun, "bun", 0o755);
  std::fs::create_dir_all(&plain).unwrap();
  let env = ChildEnv::new(Path::new("/x/toolu"), &join(&[&with_bun, &plain])).unwrap();
  let mut command = Command::new("true");
  env
    .with_codex_home(Path::new("/x/home"))
    .apply(&mut command);
  let vars: Vec<_> = command.get_envs().collect();
  let value = |key: &str| {
    vars
      .iter()
      .find(|(name, _)| *name == key)
      .map(|(_, value)| *value)
  };
  assert_eq!(
    value("TOOLU_BIN"),
    Some(Some(Path::new("/x/toolu").as_os_str()))
  );
  assert_eq!(value("TOOLU_BUN"), Some(None));
  assert_eq!(
    value("CODEX_HOME"),
    Some(Some(Path::new("/x/home").as_os_str()))
  );
  assert_eq!(value("PATH"), Some(Some(plain.as_os_str())));
}

#[test]
fn a_plain_environment_has_no_codex_home() {
  let env = ChildEnv::new(Path::new("/x/toolu"), &OsString::from("/usr/bin")).unwrap();
  let mut command = Command::new("true");
  env.apply(&mut command);
  assert!(command.get_envs().all(|(name, _)| name != "CODEX_HOME"));
}
