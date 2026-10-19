use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;

use toolu_protocol::HOOK_PROTOCOL;
use toolu_protocol::exit::Exit;
use toolu_protocol::install::INSTALLER;
use toolu_runtime::cli::Outcome;
use toolu_runtime::env::Env;

use super::run;
use crate::fast::HookRequest;
use crate::{Context, VERSION};

fn jev_home() -> (tempfile::TempDir, Env) {
  let dir = tempfile::tempdir().unwrap();
  let home = dir.path().join("home");
  let config = dir.path().join("config");
  fs::create_dir(&home).unwrap();
  let env = Env::from_pairs([
    ("HOME", home.to_str().unwrap()),
    ("TOOLU_CONFIG_DIR", config.to_str().unwrap()),
    ("PATH", dir.path().to_str().unwrap()),
  ]);
  (dir, env)
}

fn shim(dir: &Path, version: &str) {
  fs::create_dir_all(dir.join("scripts")).unwrap();
  fs::create_dir_all(dir.join(".claude-plugin")).unwrap();
  fs::write(
    dir.join("scripts/jev.sh"),
    "#!/bin/sh\nexec toolu jev \"$@\"\n",
  )
  .unwrap();
  fs::set_permissions(
    dir.join("scripts/jev.sh"),
    fs::Permissions::from_mode(0o755),
  )
  .unwrap();
  fs::write(
    dir.join(".claude-plugin/plugin.json"),
    format!(r#"{{"name":"jev","version":"{version}","hookProtocol":{HOOK_PROTOCOL}}}"#),
  )
  .unwrap();
}

fn run_jev(env: &Env, name: &str, event: &str, root: &Path, stdin: &'static str) -> Outcome {
  let exe = || None;
  let read = move || Ok(stdin.to_owned());
  let env_fn = || env.clone();
  let cwd = root.to_path_buf();
  let cwd_fn = move || Ok(cwd.clone());
  let request = HookRequest {
    plugin: "jev".to_owned(),
    name: name.to_owned(),
    event: Some(event.to_owned()),
    plugin_root: Some(root.display().to_string()),
  };
  run(
    &request,
    &Context {
      exe: &exe,
      stdin: &read,
      env: &env_fn,
      cwd: &cwd_fn,
    },
  )
}

#[test]
fn jev_session_start_joins_the_skew_advisory_to_its_context_in_one_json_object() {
  let (dir, env) = jev_home();
  shim(dir.path(), "999.0.0");
  let outcome = run_jev(&env, "session-start", "SessionStart", dir.path(), "{}");
  let body: serde_json::Value = serde_json::from_str(&outcome.stdout.unwrap()).unwrap();
  assert_eq!(
    body["systemMessage"],
    format!("toolu {VERSION} is older than the jev plugin 999.0.0; upgrade it: {INSTALLER}")
  );
  let context = body["hookSpecificOutput"]["additionalContext"]
    .as_str()
    .unwrap();
  assert!(context.contains("toolu jev"));
}

fn prefixed(stdout: &str) -> serde_json::Value {
  let outcome = Outcome {
    exit: Exit::Success,
    stdout: Some(stdout.to_owned()),
    stderr: None,
  };
  let joined = super::prefix(Some("skew".to_owned()), outcome);
  serde_json::from_str(&joined.stdout.unwrap()).unwrap()
}

#[test]
fn the_advisory_goes_ahead_of_a_hooks_own_system_message() {
  assert_eq!(
    prefixed(r#"{"systemMessage":"own","continue":true}"#),
    serde_json::json!({"systemMessage": "skew\nown", "continue": true})
  );
}

#[test]
fn a_plain_text_body_becomes_context_beside_the_advisory() {
  assert_eq!(
    prefixed("plain context"),
    serde_json::json!({
      "systemMessage": "skew",
      "hookSpecificOutput": {"hookEventName": "SessionStart", "additionalContext": "plain context"},
    })
  );
}

#[test]
fn jev_prompt_and_check_binary_go_through_the_same_dispatch() {
  let (dir, env) = jev_home();
  let root = dir.path();
  shim(root, VERSION);
  let payload = r#"{"prompt":"rank these approaches"}"#;
  run_jev(&env, "session-start", "SessionStart", root, "{}");
  let prompt = run_jev(
    &env,
    "user-prompt-submit",
    "UserPromptSubmit",
    root,
    payload,
  );
  assert!(prompt.stdout.unwrap().contains("toolu jev"));
  let id = r#"{"session_id":"s"}"#;
  assert!(
    run_jev(&env, "check-binary", "SessionStart", root, id)
      .stdout
      .unwrap()
      .contains("toolu:")
  );
  assert_eq!(
    run_jev(&env, "check-binary", "SessionStart", root, id).stdout,
    None
  );
}

#[test]
fn a_missing_plugin_root_stays_quiet_instead_of_reading_the_working_directory() {
  let (_dir, env) = jev_home();
  let exe = || None;
  let read = || Ok("{}".to_owned());
  let env_fn = || env.clone();
  let cwd_fn = || Ok(Path::new(".").to_path_buf());
  let request = HookRequest {
    plugin: "jev".to_owned(),
    name: "session-start".to_owned(),
    event: Some("SessionStart".to_owned()),
    plugin_root: None,
  };
  let outcome = run(
    &request,
    &Context {
      exe: &exe,
      stdin: &read,
      env: &env_fn,
      cwd: &cwd_fn,
    },
  );
  assert_eq!(outcome.exit, Exit::Success);
  assert_eq!(outcome.stdout, None);
  assert_eq!(outcome.stderr, None);
}
