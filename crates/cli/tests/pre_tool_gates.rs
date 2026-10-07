//! Native pre-tool gates through the real `toolu hook pre-tools` binary.

use std::fs;
use std::path::Path;
use std::process::Output;

use serde_json::{Value, json};

const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");
type Res<T> = Result<T, Box<dyn std::error::Error>>;

fn run(project: &Path, settings: &Path, host: &str, payload: &Value) -> Res<Output> {
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
  Ok(command.output()?)
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

#[test]
fn portable_core_protected_edit_asks_on_claude_and_denies_on_codex() {
  let dir = tempfile::tempdir().expect("tempdir");
  let project = dir.path().join("project");
  let settings = dir.path().join("settings");
  fs::create_dir_all(&project).expect("project");
  fs::create_dir_all(&settings).expect("settings");
  fs::write(settings.join("protected-files.txt"), ".env\n").expect("protected list");
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
    let output = run(&project, &settings, host, &payload).expect("native hook");
    assert_eq!(output.status.code(), Some(0));
    let (kind, reason) = decision(&output).expect("decision");
    assert_eq!(kind, want);
    assert!(reason.contains(".env"));
  }
}

#[test]
fn a_shell_write_and_blocked_mcp_tool_get_native_guardrails() {
  let dir = tempfile::tempdir().expect("tempdir");
  let project = dir.path().join("project");
  let settings = dir.path().join("settings");
  fs::create_dir_all(&project).expect("project");
  fs::create_dir_all(&settings).expect("settings");
  fs::write(settings.join("protected-files.txt"), ".env\n").expect("protected list");
  fs::write(settings.join("mcp-blocklist.txt"), "example -> use CLI\n").expect("mcp list");
  let bash =
    json!({"tool_name":"Bash","tool_input":{"command":"echo SECRET=1 >.env"},"session_id":"s"});
  let mcp = json!({"tool_name":"mcp__example__search","tool_input":{},"session_id":"s"});
  for (payload, text) in [(&bash, "WRITE to .env"), (&mcp, "Use instead: use CLI")] {
    let output = run(&project, &settings, "codex", payload).expect("native hook");
    assert_eq!(output.status.code(), Some(0));
    let (kind, reason) = decision(&output).expect("decision");
    assert_eq!(kind, "deny");
    assert!(reason.contains(text), "{reason}");
  }
}
