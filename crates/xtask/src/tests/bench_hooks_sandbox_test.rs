use std::os::unix::fs::PermissionsExt as _;
use std::path::PathBuf;

use super::Sandbox;
use crate::env_scrub::selects_repository;

/// A `toolu` that records each register call and succeeds.
fn fake_toolu(dir: &std::path::Path) -> PathBuf {
  let bin = dir.join("toolu");
  let log = dir.join("calls.log");
  std::fs::write(
    &bin,
    format!("#!/bin/sh\ncat >/dev/null\necho \"$1 $2 $3\" >> '{}'\n", log.display()),
  )
  .unwrap();
  std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
  bin
}

#[test]
fn the_full_fixture_registers_the_four_registry_plugins_and_is_removed_on_drop() {
  let dir = tempfile::tempdir().unwrap();
  let bin = fake_toolu(dir.path());
  let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
  let sandbox = Sandbox::new(&repo, &bin, true).unwrap();
  let project = sandbox.project.clone();
  assert!(project.join(".git").is_dir() && project.join("src/app.ts").is_file());
  let calls = std::fs::read_to_string(dir.path().join("calls.log")).unwrap();
  assert_eq!(
    calls,
    "ast-grep hook register\nts-quality hook register\npython-quality hook register\nrust-quality hook register\n"
  );
  let env = sandbox.env(&repo.join("plugins/toolu"));
  let get = |key: &str| env.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone());
  assert_eq!(get("TOOLU_BIN"), None);
  assert_eq!(
    get("CLAUDE_PROJECT_DIR"),
    Some(project.as_os_str().to_owned())
  );
  let toolu_keys = env
    .iter()
    .filter(|(k, _)| k.to_string_lossy().starts_with("TOOLU_"));
  assert_eq!(toolu_keys.count(), 0);
  assert!(env.iter().all(|(k, _)| !selects_repository(k)));
  assert!(sandbox.scratch().join("project").is_dir() && sandbox.scratch().join("home").is_dir());
  drop(sandbox);
  assert!(!project.exists());
}

#[test]
fn the_core_fixture_registers_nothing_and_a_failed_register_is_an_error() {
  let dir = tempfile::tempdir().unwrap();
  let bin = fake_toolu(dir.path());
  let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
  drop(Sandbox::new(&repo, &bin, false).unwrap());
  assert!(!dir.path().join("calls.log").exists());
  std::fs::write(&bin, "#!/bin/sh\ncat >/dev/null\necho 'no registry here' >&2\nexit 3\n").unwrap();
  let err = Sandbox::new(&repo, &bin, true).err().unwrap();
  assert_eq!(
    err,
    "bench sandbox: register ast-grep exited Some(3): no registry here"
  );
}
