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

fn hook(plugin: &str, name: &str, event: &str, env: &Env) -> toolu_runtime::cli::Outcome {
  let request = HookRequest {
    plugin: plugin.to_owned(),
    name: name.to_owned(),
    event: Some(event.to_owned()),
    plugin_root: None,
  };
  let no_exe = || None;
  let env_fn = || env.clone();
  let fallback = PathBuf::new();
  let cwd_fn = || Ok(fallback.clone());
  let context = Context {
    exe: &no_exe,
    stdin: &startup,
    env: &env_fn,
    cwd: &cwd_fn,
  };
  run(&request, &context)
}

#[test]
fn statusline_session_start_does_not_run_for_a_tool_event() {
  let outcome = hook("statusline", "session-start", "PreToolUse", &Env::default());
  assert_eq!(outcome.exit, toolu_protocol::exit::Exit::Blocked);
  assert!(
    outcome
      .stderr
      .is_some_and(|text| text.contains("has no hook session-start"))
  );
}

fn babysit(name: &str, env: &Env) -> toolu_runtime::cli::Outcome {
  hook("pr-babysit", name, "SessionStart", env)
}

#[test]
fn pr_babysit_session_start_checks_never_block_and_others_are_unknown() {
  let env = Env::default().with("TOOLU_HOST_OVERRIDE", "claude");
  let core = babysit("check-toolu", &env);
  assert_eq!(core.exit.code(), 0);
  assert_eq!(
    core.stdout, None,
    "Claude resolves plugin dependencies itself"
  );
  assert_eq!(babysit("check-binary", &env).exit.code(), 0);
  let other = babysit("babysit-tick", &env);
  assert_eq!(other.exit.code(), 0, "a context event is never blocked");
  assert!(
    other
      .stdout
      .is_some_and(|text| text.contains("has no hook babysit-tick"))
  );
}
