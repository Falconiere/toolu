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

/// The repository's root for `plugin`.
fn plugin_root(plugin: &str) -> String {
  std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../../plugins")
    .join(plugin)
    .display()
    .to_string()
}

/// Run `request` on a startup payload with `env`, from `cwd`.
fn session_start(
  request: &HookRequest,
  env: &Env,
  cwd: &std::path::Path,
) -> toolu_runtime::cli::Outcome {
  let no_exe = || None;
  let env_fn = || env.clone();
  let cwd_path = cwd.to_path_buf();
  let cwd_fn = || Ok(cwd_path.clone());
  let context = Context {
    exe: &no_exe,
    stdin: &startup,
    env: &env_fn,
    cwd: &cwd_fn,
  };
  run(request, &context)
}

#[test]
fn statusline_session_start_retires_the_compat_path_without_blocking() {
  let dir = tempfile::tempdir().expect("statusline root");
  let root = dir.path().display().to_string();
  std::fs::create_dir_all(dir.path().join("statusline")).expect("statusline dir");
  let old = format!("{}/scripts/statusline.sh", plugin_root("statusline"));
  std::os::unix::fs::symlink(old, dir.path().join("statusline/statusline.sh")).expect("old link");
  let env = Env::default()
    .with("HOME", &root)
    .with("CLAUDE_CONFIG_DIR", &root)
    .with("TOOLU_HOST_OVERRIDE", "claude");
  let other = HookRequest {
    plugin: "statusline".to_owned(),
    name: "session-start".to_owned(),
    event: Some("SessionStart".to_owned()),
    plugin_root: Some(plugin_root("statusline")),
  };
  let outcome = session_start(&other, &env, dir.path());
  assert_eq!(outcome.exit.code(), 0);
  assert_eq!(outcome.stderr, None);
  assert_eq!(outcome.stdout, None);
  assert!(std::fs::symlink_metadata(dir.path().join("statusline/statusline.sh")).is_err());
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
  let tool = hook("pr-babysit", "check-toolu", "PreToolUse", &env);
  assert_eq!(tool.exit, toolu_protocol::exit::Exit::Blocked);
  assert!(
    tool
      .stderr
      .is_some_and(|text| text.contains("has no hook check-toolu"))
  );
}

#[test]
fn epic_orchestrator_engine_ensure_runs_in_process_and_reports_a_bad_registry() {
  let dir = tempfile::tempdir().expect("epic root");
  let resources = dir.path().join("resources");
  std::fs::create_dir_all(&resources).expect("resources");
  std::fs::write(resources.join("registry.json"), "{not json").expect("registry");
  let env = Env::default()
    .with("HOME", &dir.path().display().to_string())
    .with("TOOLU_RESOURCE_HOME", &resources.display().to_string());
  let request = HookRequest {
    plugin: "epic-orchestrator".to_owned(),
    name: "engine-ensure".to_owned(),
    event: Some("SessionStart".to_owned()),
    plugin_root: Some(plugin_root("epic-orchestrator")),
  };
  let outcome = session_start(&request, &env, dir.path());
  assert_eq!(outcome.exit.code(), 0);
  let stdout = outcome.stdout.expect("message");
  assert!(
    stdout.contains(r#""systemMessage":"epic engine: "#),
    "{stdout}"
  );
}

#[test]
fn documentation_gate_registers_only_at_session_start() {
  let dir = tempfile::tempdir().unwrap();
  let env = Env::default()
    .with("HOME", &dir.path().display().to_string())
    .with("TOOLU_CONFIG_DIR", &dir.path().display().to_string())
    .with("TOOLU_HOST_OVERRIDE", "claude");
  let outcome = hook("documentation-gate", "register", "SessionStart", &env);
  assert_eq!(outcome.exit.code(), 0);
  assert!(
    dir
      .path()
      .join("toolu/pre-tools.d/documentation-gate@toolu__docs-review.json")
      .is_file()
  );
  assert_eq!(
    hook("documentation-gate", "check-toolu", "SessionStart", &env)
      .exit
      .code(),
    0
  );
  assert_eq!(
    hook("documentation-gate", "register", "PreToolUse", &env).exit,
    toolu_protocol::exit::Exit::Blocked
  );
}

#[test]
fn git_better_registers_only_at_session_start() {
  let dir = tempfile::tempdir().unwrap();
  let env = Env::default()
    .with("HOME", &dir.path().display().to_string())
    .with("TOOLU_CONFIG_DIR", &dir.path().display().to_string())
    .with("TOOLU_HOST_OVERRIDE", "claude");
  let outcome = hook("git-better", "register", "SessionStart", &env);
  assert_eq!(outcome.exit.code(), 0);
  assert!(
    dir
      .path()
      .join("toolu/pre-tools.d/git-better@toolu__semantic-git.json")
      .is_file()
  );
  assert_eq!(
    hook("git-better", "check-toolu", "SessionStart", &env)
      .exit
      .code(),
    0
  );
  assert_eq!(
    hook("git-better", "register", "PreToolUse", &env).exit,
    toolu_protocol::exit::Exit::Blocked
  );
}
