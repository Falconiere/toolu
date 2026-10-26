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
    format!(
      "#!/bin/sh\ncat >/dev/null\necho \"$1 $2 $3${{MUSE_PLUGIN_ID:+ muse}}\" >> '{}'\n",
      log.display()
    ),
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
    "ast-grep hook register\nast-grep hook register muse\nts-quality hook register\n\
     ts-quality hook register muse\npython-quality hook register\n\
     python-quality hook register muse\nrust-quality hook register\nrust-quality hook register muse\n"
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
  std::fs::write(
    &bin,
    "#!/bin/sh\ncat >/dev/null\necho 'no registry here' >&2\nexit 3\n",
  )
  .unwrap();
  let err = Sandbox::new(&repo, &bin, true).err().unwrap();
  assert_eq!(
    err,
    "bench sandbox: register ast-grep exited Some(3): no registry here"
  );
}

#[test]
fn a_muse_entry_gets_only_what_muse_forwards_plus_the_measured_binary() {
  let dir = tempfile::tempdir().unwrap();
  let bin = fake_toolu(dir.path());
  let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
  let sandbox = Sandbox::new(&repo, &bin, false).unwrap();
  let root = repo.join("plugins/jev");
  let env = sandbox.muse_env(&root, "jev");
  let get = |key: &str| env.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone());
  assert_eq!(get("MUSE_PLUGIN_ID"), Some("jev".into()));
  for key in ["MUSE_PLUGIN_ROOT", "CLAUDE_PLUGIN_ROOT", "PLUGIN_ROOT"] {
    assert_eq!(get(key), Some(root.as_os_str().to_owned()), "{key}");
  }
  let data = get("MUSE_PLUGIN_DATA_DIR").unwrap();
  assert_eq!(get("CLAUDE_PLUGIN_DATA"), Some(data.clone()));
  assert_eq!(get("PLUGIN_DATA"), Some(data));
  assert_eq!(get("MUSE_SESSION_ID"), Some("hook-bench".into()));
  assert_eq!(get("TOOLU_BIN"), Some(bin.as_os_str().to_owned()));
  assert_eq!(get("PWD"), Some(sandbox.project.as_os_str().to_owned()));
  let allowed = [
    "HOME",
    "LANG",
    "LOGNAME",
    "PATH",
    "PWD",
    "SHELL",
    "TERM",
    "USER",
    "MUSE_PLUGIN_ID",
    "MUSE_PLUGIN_ROOT",
    "MUSE_PLUGIN_DATA_DIR",
    "MUSE_SESSION_ID",
    "MUSE_THREAD_ID",
    "CLAUDE_PLUGIN_ROOT",
    "CLAUDE_PLUGIN_DATA",
    "PLUGIN_ROOT",
    "PLUGIN_DATA",
    "TOOLU_BIN",
  ];
  let extra: Vec<_> = env
    .iter()
    .filter(|(k, _)| !allowed.iter().any(|name| k == name))
    .collect();
  assert!(extra.is_empty(), "{extra:?}");
  assert!(get("PATH").is_some() && get("HOME").is_some());
}

#[test]
fn a_register_that_reports_failure_at_exit_0_is_an_error() {
  let dir = tempfile::tempdir().unwrap();
  let bin = fake_toolu(dir.path());
  let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
  std::fs::write(
    &bin,
    "#!/bin/sh\ncat >/dev/null\necho '{\"systemMessage\":\"ast-grep registration failed: read-only\"}'\n",
  )
  .unwrap();
  let err = Sandbox::new(&repo, &bin, true).err().unwrap();
  assert_eq!(
    err,
    "bench sandbox: register ast-grep: {\"systemMessage\":\"ast-grep registration failed: read-only\"}"
  );
}
