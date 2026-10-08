use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;

use toolu_runtime::env::Env;

use super::check_binary;
use crate::hooks::{Sandbox, assert_silent, context_of};

fn toolu(dir: &Path, body: &str) {
  fs::create_dir_all(dir).unwrap();
  let path = dir.join("toolu");
  fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
  fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
}

fn env(sb: &Sandbox, path: &Path) -> Env {
  let config = sb.dir.path().join("config");
  Env::from_pairs([
    ("HOME", sb.home.to_str().unwrap()),
    ("TOOLU_CONFIG_DIR", config.to_str().unwrap()),
    ("PATH", path.to_str().unwrap()),
  ])
}

#[test]
fn a_native_toolu_in_the_shell_prints_nothing() {
  let sb = Sandbox::new();
  let bin = sb.dir.path().join("bin");
  toolu(&bin, "if [ \"$1\" = --hook-protocol ]; then echo 1; fi");
  assert_silent(&check_binary(
    &env(&sb, &bin),
    Some(r#"{"session_id":"s"}"#),
  ));
}

#[test]
fn without_toolu_one_install_line_is_printed_as_session_context_and_once_per_id() {
  let sb = Sandbox::new();
  let env = env(&sb, &sb.dir.path().join("empty"));
  let first = check_binary(&env, Some(r#"{"session_id":"abc"}"#));
  let line = context_of(&first, "SessionStart");
  assert!(line.starts_with("toolu: native binary not found in the agent command shell."));
  assert!(line.contains("curl -fsSL https://get.toolu.sh/pkg/toolu/install | bash"));
  assert!(line.contains("brew install falconiere/tap/toolu"));
  assert_silent(&check_binary(&env, Some(r#"{"session_id":"abc"}"#)));
  context_of(
    &check_binary(&env, Some(r#"{"session_id":"other"}"#)),
    "SessionStart",
  );
}

#[test]
fn a_missing_session_id_prints_every_time_and_the_environment_id_is_the_fallback() {
  let sb = Sandbox::new();
  let env = env(&sb, &sb.dir.path().join("empty"));
  for stdin in [
    None,
    Some(""),
    Some("bad json"),
    Some("[]"),
    Some(r#"{"session_id":7}"#),
  ] {
    context_of(&check_binary(&env, stdin), "SessionStart");
    context_of(&check_binary(&env, stdin), "SessionStart");
  }
  let env = env.with("TOOLU_SESSION_ID", "from-env");
  context_of(&check_binary(&env, Some("{}")), "SessionStart");
  assert_silent(&check_binary(&env, Some("{}")));
  assert_silent(&check_binary(&env, None));
  let payload_wins = check_binary(&env, Some(r#"{"session_id":"from-payload"}"#));
  context_of(&payload_wins, "SessionStart");
}

#[test]
fn a_shadowing_non_native_toolu_still_gets_the_line_with_the_known_path() {
  let sb = Sandbox::new();
  let bin = sb.dir.path().join("bin");
  toolu(&bin, "echo not-a-protocol");
  let known = sb.dir.path().join("known");
  toolu(&known, "if [ \"$1\" = --hook-protocol ]; then echo 1; fi");
  let env = env(&sb, &bin).with("TOOLU_BIN", known.join("toolu").to_str().unwrap());
  let line = context_of(&check_binary(&env, Some("{}")), "SessionStart");
  let path = known.join("toolu");
  assert!(line.contains(&format!(
    "native binary for this session: '{}'",
    path.display()
  )));
}
