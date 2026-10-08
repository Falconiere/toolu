use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use super::native_toolu_advice;
use crate::env::Env;

fn script(dir: &Path, body: &str) -> PathBuf {
  let path = dir.join("toolu");
  fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
  fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
  path
}

fn sandbox() -> (tempfile::TempDir, Env) {
  let dir = tempfile::tempdir().unwrap();
  let home = dir.path().join("home with 'quote'");
  let config = dir.path().join("config");
  let bin = dir.path().join("bin");
  fs::create_dir(&home).unwrap();
  fs::create_dir(&config).unwrap();
  fs::create_dir(&bin).unwrap();
  let env = Env::from_pairs([
    ("HOME", home.to_str().unwrap()),
    ("TOOLU_CONFIG_DIR", config.to_str().unwrap()),
    ("PATH", bin.to_str().unwrap()),
  ]);
  (dir, env)
}

#[test]
fn a_native_script_on_path_prints_nothing() {
  let (dir, mut env) = sandbox();
  let bin = dir.path().join("bin");
  script(&bin, "if [ \"$1\" = --hook-protocol ]; then echo 1; fi");
  env = env.with("PATH", bin.to_str().unwrap());
  assert_eq!(native_toolu_advice(&env, Some("native")), None);
}

#[test]
fn a_binary_outside_path_is_quoted_once() {
  let (dir, env) = sandbox();
  let local = dir.path().join("home with 'quote'").join(".local/bin");
  fs::create_dir_all(&local).unwrap();
  script(&local, "if [ \"$1\" = --hook-protocol ]; then echo 1; fi");
  let first = native_toolu_advice(&env, Some("local")).unwrap();
  assert!(first.contains(".local/bin/toolu"));
  assert!(first.contains("native binary for this session:"));
  assert!(first.contains("quote'\"'\"'"));
  assert_eq!(native_toolu_advice(&env, Some("local")), None);
}

#[test]
fn no_binary_prints_both_install_commands_once() {
  let (_dir, env) = sandbox();
  let first = native_toolu_advice(&env, Some("missing")).unwrap();
  assert_eq!(
    first,
    "toolu: native binary not found. Install it with `curl -fsSL https://get.toolu.sh/pkg/toolu/install | bash` or `brew install falconiere/tap/toolu`, then use `toolu` directly."
  );
  assert_eq!(native_toolu_advice(&env, Some("missing")), None);
}

#[test]
fn a_missing_session_id_prints_every_time() {
  let (_dir, env) = sandbox();
  let first = native_toolu_advice(&env, None).unwrap();
  assert_eq!(native_toolu_advice(&env, Some("")), Some(first));
}

#[test]
fn a_wrapper_on_path_yields_to_a_known_native_binary() {
  let (dir, mut env) = sandbox();
  script(&dir.path().join("bin"), "exit 1");
  let local = dir.path().join("home with 'quote'").join(".local/bin");
  fs::create_dir_all(&local).unwrap();
  script(&local, "if [ \"$1\" = --hook-protocol ]; then echo 1; fi");
  env = env.with("PATH", dir.path().join("bin").to_str().unwrap());
  let advice = native_toolu_advice(&env, Some("wrapper")).unwrap();
  assert!(advice.contains(".local/bin/toolu"));
  assert!(!advice.contains("not found"));
}
