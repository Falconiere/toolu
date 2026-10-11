use std::path::PathBuf;

use toolu_runtime::env::Env;

use super::super::run;
use crate::Context;
use crate::fast::HookRequest;

fn startup() -> std::io::Result<String> {
  toolu_protocol::stdin::read_all(
    r#"{"hook_event_name":"SessionStart","source":"startup"}"#.as_bytes(),
  )
}

#[test]
fn statusline_session_start_publishes_compat_path_without_blocking() {
  let dir = tempfile::tempdir().expect("statusline root");
  let root = dir.path().display().to_string();
  let env = Env::default()
    .with("HOME", &root)
    .with("CLAUDE_CONFIG_DIR", &root)
    .with("TOOLU_HOST_OVERRIDE", "claude");
  let other = HookRequest {
    plugin: "statusline".to_owned(),
    name: "session-start".to_owned(),
    event: Some("SessionStart".to_owned()),
    plugin_root: Some(
      std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../plugins/statusline")
        .display()
        .to_string(),
    ),
  };
  let no_exe = || None;
  let env_fn = || env.clone();
  let cwd_path = dir.path().to_path_buf();
  let cwd_fn = || Ok(cwd_path.clone());
  let context = Context {
    exe: &no_exe,
    stdin: &startup,
    env: &env_fn,
    cwd: &cwd_fn,
  };
  let outcome = run(&other, &context);
  assert_eq!(outcome.exit.code(), 0);
  assert_eq!(outcome.stderr, None);
  assert_eq!(outcome.stdout, None);
  assert!(dir.path().join("statusline/statusline.sh").is_symlink());
}

#[test]
fn statusline_session_start_does_not_run_for_a_tool_event() {
  let other = HookRequest {
    plugin: "statusline".to_owned(),
    name: "session-start".to_owned(),
    event: Some("PreToolUse".to_owned()),
    plugin_root: None,
  };
  let no_exe = || None;
  let env = Env::default();
  let env_fn = || env.clone();
  let fallback = PathBuf::new();
  let cwd_fn = || Ok(fallback.clone());
  let context = Context {
    exe: &no_exe,
    stdin: &startup,
    env: &env_fn,
    cwd: &cwd_fn,
  };
  let outcome = run(&other, &context);
  assert_eq!(outcome.exit, toolu_protocol::exit::Exit::Blocked);
  assert!(
    outcome
      .stderr
      .is_some_and(|text| text.contains("has no hook session-start"))
  );
}
