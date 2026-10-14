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

fn names(dir: &Path) -> Vec<OsString> {
  let listing = std::fs::read_dir(dir).unwrap();
  let mut found: Vec<OsString> = listing.flatten().map(|entry| entry.file_name()).collect();
  found.sort_unstable();
  found
}

#[test]
fn a_directory_with_bun_is_replaced_by_links_to_its_other_tools() {
  let dir = tempfile::tempdir().unwrap();
  let brew = dir.path().join("brew");
  let plain = dir.path().join("plain");
  let inert = dir.path().join("inert");
  let shims = dir.path().join("shims");
  for name in ["bun", "bunx", "git", "node", "data"] {
    script(&brew, name, if name == "data" { 0o644 } else { 0o755 });
  }
  script(&plain, "git", 0o755);
  script(&plain, "rg", 0o755);
  script(&inert, "bun", 0o644);
  let filtered = path_without_bun(&join(&[&plain, &brew, &inert]), &shims).unwrap();
  let kept: Vec<_> = std::env::split_paths(&filtered).collect();
  let shim = shims.join("0");
  assert_eq!(kept, vec![plain, shim.clone(), inert]);
  assert_eq!(names(&shim), ["git", "node"]);
  let linked = std::fs::read_link(shim.join("git")).unwrap();
  assert_eq!(linked, brew.join("git"));
  assert!(find_tool("git", &filtered).is_some() && find_tool("bun", &filtered).is_none());
}

#[test]
fn non_adjacent_directories_with_bun_keep_their_slots_and_their_own_tools() {
  let dir = tempfile::tempdir().unwrap();
  let (first, plain, last) = (
    dir.path().join("first"),
    dir.path().join("plain"),
    dir.path().join("last"),
  );
  let shims = dir.path().join("shims");
  for tool in ["bun", "git", "only-first"] {
    script(&first, tool, 0o755);
  }
  for tool in ["git", "rg"] {
    script(&plain, tool, 0o755);
  }
  for tool in ["bunx", "git", "only-last"] {
    script(&last, tool, 0o755);
  }
  let filtered = path_without_bun(&join(&[&first, &plain, &last]), &shims).unwrap();
  let kept: Vec<_> = std::env::split_paths(&filtered).collect();
  assert_eq!(kept, vec![shims.join("0"), plain.clone(), shims.join("1")]);
  assert_eq!(names(&shims.join("0")), ["git", "only-first"]);
  assert_eq!(names(&shims.join("1")), ["git", "only-last"]);
  assert_eq!(
    find_tool("git", &filtered).unwrap(),
    shims.join("0").join("git")
  );
  assert_eq!(
    std::fs::read_link(shims.join("1").join("git")).unwrap(),
    last.join("git")
  );
  let plain_path = join(&[&plain, &shims.join("1")]);
  assert_eq!(find_tool("git", &plain_path).unwrap(), plain.join("git"));
  assert!(find_tool("bun", &filtered).is_none() && find_tool("bunx", &filtered).is_none());
}

#[test]
fn a_path_without_bun_is_unchanged_and_creates_no_directory() {
  let dir = tempfile::tempdir().unwrap();
  let plain = dir.path().join("plain");
  let shims = dir.path().join("shims");
  script(&plain, "git", 0o755);
  let path = join(&[&plain]);
  assert_eq!(path_without_bun(&path, &shims).unwrap(), path);
  assert!(!shims.exists());
  assert_eq!(
    path_without_bun(&OsString::new(), &shims).unwrap(),
    OsString::new()
  );
}

#[test]
fn an_existing_shim_directory_is_refused() {
  let dir = tempfile::tempdir().unwrap();
  let brew = dir.path().join("brew");
  script(&brew, "bun", 0o755);
  let shims = dir.path().join("shims");
  std::fs::create_dir(&shims).unwrap();
  let err = path_without_bun(&join(&[&brew]), &shims).unwrap_err();
  assert!(err.contains("cannot create"), "{err}");
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
  let shims = dir.path().join("shims");
  let env = ChildEnv::new(Path::new("/x/toolu"), &join(&[&bin_dir]), &shims).unwrap();
  assert_eq!(env.tool("claude").unwrap(), bin_dir.join("claude"));
  assert!(env.tool("codex").unwrap_err().contains("codex"));
}

#[test]
fn apply_sets_the_native_binary_and_drops_the_bun_and_git_selection_overrides() {
  let dir = tempfile::tempdir().unwrap();
  let with_bun = dir.path().join("with-bun");
  let plain = dir.path().join("plain");
  let shims = dir.path().join("shims");
  script(&with_bun, "bun", 0o755);
  std::fs::create_dir_all(&plain).unwrap();
  let path = join(&[&with_bun, &plain]);
  let env = ChildEnv::new(Path::new("/x/toolu"), &path, &shims).unwrap();
  let mut command = Command::new("true");
  command
    .env("GIT_DIR", "/elsewhere/.git")
    .env("GIT_INDEX_FILE", "/elsewhere/index");
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
  assert_eq!(value("GIT_DIR"), Some(None));
  assert_eq!(value("GIT_INDEX_FILE"), Some(None));
  assert_eq!(value("GIT_WORK_TREE"), Some(None));
  assert_eq!(
    value("CODEX_HOME"),
    Some(Some(Path::new("/x/home").as_os_str()))
  );
  let expected = join(&[&shims.join("0"), &plain]);
  assert_eq!(value("PATH"), Some(Some(expected.as_os_str())));
}

#[test]
fn a_plain_environment_has_no_codex_home() {
  let dir = tempfile::tempdir().unwrap();
  let shims = dir.path().join("shims");
  let env = ChildEnv::new(Path::new("/x/toolu"), &OsString::from("/usr/bin"), &shims).unwrap();
  let mut command = Command::new("true");
  env.apply(&mut command);
  assert!(command.get_envs().all(|(name, _)| name != "CODEX_HOME"));
}
