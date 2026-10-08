use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;

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
    format!(r#"{{"name":"jev","version":"{version}","hookProtocol":1}}"#),
  )
  .unwrap();
}

fn run_jev(env: &Env, name: &str, event: &str, root: &Path, stdin: &'static str) -> Outcome {
  let exe = || None;
  let read = move || Ok(stdin.to_owned());
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
      env: Some(env),
    },
  )
}

#[test]
fn jev_session_start_prints_context_and_a_skew_line_before_it() {
  let (dir, env) = jev_home();
  shim(dir.path(), "999.0.0");
  let outcome = run_jev(&env, "session-start", "SessionStart", dir.path(), "{}");
  let stdout = outcome.stdout.unwrap();
  let mut lines = stdout.lines();
  let advisory: serde_json::Value = serde_json::from_str(lines.next().unwrap()).unwrap();
  let body: serde_json::Value = serde_json::from_str(lines.next().unwrap()).unwrap();
  assert!(
    advisory["systemMessage"]
      .as_str()
      .unwrap()
      .contains("999.0.0")
  );
  let context = body["hookSpecificOutput"]["additionalContext"]
    .as_str()
    .unwrap();
  assert!(context.contains("toolu jev") && lines.next().is_none());
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
