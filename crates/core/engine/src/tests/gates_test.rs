use std::path::Path;

use serde_json::{Map, json};
use toolu_protocol::host::Host;
use toolu_protocol::normalized::{NormalizedEvent, Session, Tool};
use toolu_protocol::text::Text;
use toolu_runtime::env::Env;
use toolu_runtime::registry::rule::RuleContext;

use super::{command_analysis, is_shell_tool};

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
