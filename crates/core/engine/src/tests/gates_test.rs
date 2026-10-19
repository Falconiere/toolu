use std::fs;
use std::path::Path;

use serde_json::{Map, json};
use toolu_protocol::host::Host;
use toolu_protocol::normalized::{NormalizedEvent, Session, Tool};
use toolu_protocol::text::Text;
use toolu_runtime::env::Env;
use toolu_runtime::registry::rule::RuleContext;

use super::{bash_commands::BASH_COMMANDS, commit_gate::COMMIT_GATE};
use super::{command_analysis, file_path, is_shell_tool};
use crate::gate::Gate;
use toolu_protocol::decision::Decision;

fn text(value: &str) -> Text {
  Text::new(value).unwrap()
}

fn post(tool: &str) -> NormalizedEvent {
  NormalizedEvent::ToolPost {
    session: Session {
      session_id: text("s"),
      cwd: text("/p"),
      project_root: text("/p"),
      worktree: text("/p"),
    },
    tool: Tool {
      call_id: text("c"),
      name: text(tool),
      input: Map::new(),
    },
    output: None,
  }
}

#[test]
fn bash_and_shell_are_the_shell_tools() {
  assert!(is_shell_tool(&post("Bash")));
  assert!(is_shell_tool(&post("Shell")));
  assert!(!is_shell_tool(&post("Write")));
  assert!(!is_shell_tool(&post("bash")));
}

#[test]
fn muse_edit_and_write_paths_are_read_from_path() {
  for name in ["Edit", "Write"] {
    let mut event = post(name);
    if let NormalizedEvent::ToolPost { tool, .. } = &mut event {
      tool.input.insert("path".to_owned(), json!("src/main.py"));
    }
    assert_eq!(file_path(&event), "src/main.py", "{name}");
  }
  assert_eq!(file_path(&post("Edit")), "");
}

#[test]
fn the_analysis_is_of_the_reported_command() {
  let env = Env::from_pairs([("HOME", "/h")]);
  let raw = json!({"tool_input": {"command": "cd x && cargo test | tail"}})
    .as_object()
    .cloned()
    .unwrap_or_default();
  let ctx = RuleContext {
    host: Host::Claude,
    env: &env,
    config_root: Path::new("/h/.claude"),
    project_root: Path::new("/p"),
    cwd: None,
    plugin_root: None,
    raw: &raw,
    edit: None,
  };
  let analysis = command_analysis(&ctx);
  assert_eq!(analysis.commands.len(), 3);
  assert!(
    !analysis
      .commands
      .iter()
      .any(|command| command.exit_proves && command.text.starts_with("cargo"))
  );
}

#[test]
fn muse_shell_commands_keep_commit_and_denylist_enforcement() {
  let dir = tempfile::tempdir().expect("tempdir");
  fs::write(dir.path().join("bash-denylist.txt"), "node -e\n").expect("denylist");
  fs::write(dir.path().join("commit-prefixes.txt"), "feat\nfix\n").expect("prefixes");
  fs::write(
    dir.path().join("toolu.config.json"),
    r#"{"version":1,"gates":{"bashCommands":{"mode":"block"},"commitGate":{"mode":"block"}}}"#,
  )
  .expect("config");
  let home = dir.path().to_string_lossy().into_owned();
  let env = Env::from_pairs([
    ("TOOLU_SETTINGS_DIR", home.as_str()),
    ("TOOLU_USER_CONFIG_DIR", home.as_str()),
    ("HOME", home.as_str()),
  ]);
  for (command, bash_deny, commit_deny) in [
    ("node -e 1", true, false),
    ("git commit -m 'wibble: x'", false, true),
    ("echo ready", false, false),
  ] {
    let input = json!({"command":command});
    let event: NormalizedEvent = serde_json::from_value(json!({
      "type":"shell/pre", "sessionId":"s", "cwd":home, "projectRoot":home,
      "worktree":home, "toolCallId":"c", "toolName":"Bash",
      "toolInput":input, "command":command
    }))
    .expect("canonical Muse shell");
    let raw = json!({"tool_input":input})
      .as_object()
      .cloned()
      .expect("payload");
    let ctx = RuleContext {
      host: Host::Muse,
      env: &env,
      config_root: dir.path(),
      project_root: dir.path(),
      cwd: Some(dir.path()),
      plugin_root: None,
      raw: &raw,
      edit: None,
    };
    let bash = BASH_COMMANDS.run(&event, &ctx).expect("bash decision");
    let commit = COMMIT_GATE.run(&event, &ctx).expect("commit decision");
    assert_eq!(
      matches!(bash, Decision::Deny { .. }),
      bash_deny,
      "{command}"
    );
    assert_eq!(
      matches!(commit, Decision::Deny { .. }),
      commit_deny,
      "{command}"
    );
  }
}
