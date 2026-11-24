//! Native pre-tool gates through the real `toolu hook pre-tools` binary.

use std::fs;
use std::path::Path;
use std::process::Output;

use serde_json::{Value, json};

const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");
type Res<T> = Result<T, Box<dyn std::error::Error>>;

/// `hook pre-tools` for `host`, reading settings from `settings` and, when
/// `config` is `Some`, an explicit `toolu.config.json` directory.
fn run_with(
  project: &Path,
  settings: &Path,
  config: Option<&Path>,
  host: &str,
  payload: &Value,
) -> Res<Output> {
  let mut command = assert_cmd::Command::new(TOOLU);
  command
    .args(["hook", "pre-tools", "--event", "PreToolUse"])
    .env_clear()
    .env("PATH", std::env::var("PATH")?)
    .env("HOME", project.parent().ok_or("project has no parent")?)
    .env("TOOLU_HOST_OVERRIDE", host)
    .env("TOOLU_SETTINGS_DIR", settings)
    .env("CLAUDE_PROJECT_DIR", project)
    .current_dir(project)
    .write_stdin(payload.to_string());
  if let Some(config) = config {
    command.env("TOOLU_USER_CONFIG_DIR", config);
  }
  Ok(command.output()?)
}

/// `hook pre-tools` for `host` under the shipped (balanced) defaults.
fn run(project: &Path, settings: &Path, host: &str, payload: &Value) -> Res<Output> {
  run_with(project, settings, None, host, payload)
}

fn decision(output: &Output) -> Res<(String, String)> {
  let doc: Value = serde_json::from_slice(&output.stdout)?;
  let hook = doc.get("hookSpecificOutput").ok_or("no hook decision")?;
  let kind = hook
    .get("permissionDecision")
    .and_then(Value::as_str)
    .unwrap_or("");
  let reason = hook
    .get("permissionDecisionReason")
    .and_then(Value::as_str)
    .unwrap_or("");
  Ok((kind.to_owned(), reason.to_owned()))
}

/// A `settings` dir plus a `toolu.config.json` in a fresh config dir.
fn write_config(dir: &Path, doc: &Value) -> Res<std::path::PathBuf> {
  let config = dir.join("config");
  fs::create_dir_all(&config)?;
  fs::write(config.join("toolu.config.json"), format!("{doc}\n"))?;
  Ok(config)
}

/// The guardrail mechanism: with `protectedFiles` pinned to `ask`, Claude is
/// prompted and Codex, which cannot prompt, fails closed to a deny. The
/// shipped default is `advise`; the `ask` degrade path is what this pins.
#[test]
fn portable_core_protected_edit_asks_on_claude_and_denies_on_codex() {
  let dir = tempfile::tempdir().expect("tempdir");
  let project = dir.path().join("project");
  let settings = dir.path().join("settings");
  fs::create_dir_all(&project).expect("project");
  fs::create_dir_all(&settings).expect("settings");
  fs::write(settings.join("protected-files.txt"), ".env\n").expect("protected list");
  let config = write_config(
    dir.path(),
    &json!({"version": 1, "gates": {"protectedFiles": {"mode": "ask"}}}),
  )
  .expect("config");
  let fixture: Value = serde_json::from_str(include_str!(
    "../../../fixtures/portable-core/protected-files-pre.json"
  ))
  .expect("portable fixture");
  let relative = fixture
    .get("toolInput")
    .and_then(|input| input.get("file_path"))
    .and_then(Value::as_str)
    .and_then(|path| path.strip_prefix("/repo/"))
    .expect("portable fixture path");
  let payload = json!({"tool_name":fixture.get("toolName"),
    "tool_input":{"file_path":project.join(relative)},"session_id":"portable"});
  for (host, want) in [("claude", "ask"), ("codex", "deny")] {
    let output = run_with(&project, &settings, Some(&config), host, &payload).expect("native hook");
    assert_eq!(output.status.code(), Some(0));
    let (kind, reason) = decision(&output).expect("decision");
    assert_eq!(kind, want);
    assert!(reason.contains(".env"));
  }
}

/// With `protectedFiles` pinned to `block`, a shell redirect that would write
/// a protected path is denied outright.
#[test]
fn a_shell_write_to_env_gets_the_native_guardrail() {
  let dir = tempfile::tempdir().expect("tempdir");
  let project = dir.path().join("project");
  let settings = dir.path().join("settings");
  fs::create_dir_all(&project).expect("project");
  fs::create_dir_all(&settings).expect("settings");
  fs::write(settings.join("protected-files.txt"), ".env\n").expect("protected list");
  let config = write_config(
    dir.path(),
    &json!({"version": 1, "gates": {"protectedFiles": {"mode": "block"}}}),
  )
  .expect("config");
  let bash =
    json!({"tool_name":"Bash","tool_input":{"command":"echo SECRET=1 >.env"},"session_id":"s"});
  let output = run_with(&project, &settings, Some(&config), "codex", &bash).expect("native hook");
  assert_eq!(output.status.code(), Some(0));
  let (kind, reason) = decision(&output).expect("decision");
  assert_eq!(kind, "deny");
  assert!(reason.contains("WRITE to .env"), "{reason}");
}

/// What ships: the balanced default advises — the agent is told, nothing is
/// held, and the file is written.
#[test]
fn protected_shell_write_advises_by_default() {
  let dir = tempfile::tempdir().expect("tempdir");
  let project = dir.path().join("project");
  let settings = dir.path().join("settings");
  fs::create_dir_all(&project).expect("project");
  fs::create_dir_all(&settings).expect("settings");
  fs::write(settings.join("protected-files.txt"), ".env\n").expect("protected list");
  fs::write(project.join(".env"), "KEEP=1\n").expect("seed");
  let bash =
    json!({"tool_name":"Bash","tool_input":{"command":"echo SECRET=1 >.env"},"session_id":"s"});
  let output = run(&project, &settings, "codex", &bash).expect("native hook");
  assert_eq!(output.status.code(), Some(0));
  let doc: Value = serde_json::from_slice(&output.stdout).expect("advisory");
  let text = doc["hookSpecificOutput"]["additionalContext"]
    .as_str()
    .expect("advisory text");
  assert!(text.contains(".env"), "{text}");
  assert!(
    doc["hookSpecificOutput"]
      .get("permissionDecision")
      .is_none()
  );
}
