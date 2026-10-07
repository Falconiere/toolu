use std::fs;
use std::path::Path;

use serde_json::{Map, Value, json};
use toolu_protocol::decision::Decision;
use toolu_protocol::host::Host;
use toolu_protocol::normalized::NormalizedEvent;
use toolu_runtime::env::Env;
use toolu_runtime::registry::rule::RuleContext;

use super::ProtectedFiles;
use crate::gate::Gate;

fn event(
  tool: &str,
  command: Option<&str>,
  path: Option<&str>,
) -> (NormalizedEvent, Map<String, Value>) {
  let input = command.map_or_else(
    || json!({"file_path":path}),
    |command| json!({"command":command}),
  );
  let kind = if command.is_some() {
    "shell/pre"
  } else {
    "tool/pre"
  };
  let mut wire = json!({
    "type": kind, "sessionId":"s", "cwd":"/tmp", "projectRoot":"/tmp",
    "worktree":"/tmp", "toolCallId":"c", "toolName":tool, "toolInput":input
  });
  if let Some(command) = command {
    wire
      .as_object_mut()
      .expect("object")
      .insert("command".to_owned(), json!(command));
  }
  let mut raw = Map::new();
  raw.insert("tool_input".to_owned(), input);
  (serde_json::from_value(wire).expect("normalized event"), raw)
}

#[test]
fn env_edit_asks_on_claude_and_denies_on_codex() {
  let dir = tempfile::tempdir().expect("tempdir");
  fs::write(dir.path().join("protected-files.txt"), ".env\n").expect("settings");
  let settings = dir.path().to_string_lossy().into_owned();
  let env = Env::from_pairs([
    ("TOOLU_SETTINGS_DIR", settings.as_str()),
    ("HOME", settings.as_str()),
  ]);
  let (event, raw) = event("Edit", None, Some(".env"));
  for (host, ask) in [(Host::Claude, true), (Host::Codex, false)] {
    let ctx = RuleContext {
      host,
      env: &env,
      config_root: dir.path(),
      project_root: dir.path(),
      cwd: Some(dir.path()),
      plugin_root: None,
      raw: &raw,
      edit: None,
    };
    let decision = ProtectedFiles.run(&event, &ctx).expect("decision");
    match (decision, ask) {
      (Decision::Ask { reason }, true) => assert!(reason.as_str().contains("SECURITY GUARDRAIL")),
      (Decision::Deny { reason }, false) => assert!(reason.as_str().contains(".env")),
      _ => panic!("wrong host decision"),
    }
  }
}

#[test]
fn real_shell_redirect_to_env_is_guarded_but_read_is_allowed() {
  let dir = tempfile::tempdir().expect("tempdir");
  fs::write(dir.path().join("protected-files.txt"), ".env\n").expect("settings");
  let settings = dir.path().to_string_lossy().into_owned();
  let env = Env::from_pairs([
    ("TOOLU_SETTINGS_DIR", settings.as_str()),
    ("HOME", settings.as_str()),
  ]);
  for (command, guarded) in [("echo SECRET=1 >.env", true), ("cat .env", false)] {
    let (event, raw) = event("Bash", Some(command), None);
    let ctx = RuleContext {
      host: Host::Claude,
      env: &env,
      config_root: dir.path(),
      project_root: dir.path(),
      cwd: Some(Path::new("/tmp")),
      plugin_root: None,
      raw: &raw,
      edit: None,
    };
    let decision = ProtectedFiles.run(&event, &ctx).expect("decision");
    assert_eq!(
      matches!(decision, Decision::Ask { .. }),
      guarded,
      "{command}"
    );
  }
}
