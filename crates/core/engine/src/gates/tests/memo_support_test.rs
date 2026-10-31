use std::cell::Cell;
use std::path::Path;

use serde_json::{Map, Value, json};
use toolu_protocol::host::Host;
use toolu_protocol::normalized::{NormalizedEvent, Session, Tool};
use toolu_protocol::text::Text;
use toolu_runtime::env::Env;
use toolu_runtime::registry::rule::RuleContext;

pub(super) fn reset_counts() {
  super::PARSES.with(|count| count.set(0));
  super::LOADS.with(|count| count.set(0));
}

pub(super) fn analysis_parses() -> usize {
  super::PARSES.with(Cell::get)
}

pub(super) fn config_loads() -> usize {
  super::LOADS.with(Cell::get)
}

fn text(value: &str) -> Text {
  Text::new(value).expect("text")
}

pub(super) fn command_raw(command: &str) -> Map<String, Value> {
  json!({"tool_input": {"command": command}})
    .as_object()
    .expect("object")
    .clone()
}

pub(super) fn bash_session() -> Session {
  Session {
    session_id: text("s"),
    cwd: text("cwd"),
    project_root: text("root"),
    worktree: text("tree"),
  }
}

pub(super) fn bash_tool() -> Tool {
  Tool {
    call_id: text("c"),
    name: text("Bash"),
    input: Map::new(),
  }
}

pub(super) fn shell(command: &str) -> (NormalizedEvent, Map<String, Value>) {
  let event = NormalizedEvent::ShellPre {
    session: bash_session(),
    tool: bash_tool(),
    command: text(command),
  };
  (event, command_raw(command))
}

pub(super) fn context<'a>(
  env: &'a Env,
  root: &'a Path,
  raw: &'a Map<String, Value>,
  cwd: Option<&'a Path>,
  host: Host,
) -> RuleContext<'a> {
  RuleContext {
    host,
    env,
    config_root: root,
    project_root: root,
    cwd,
    plugin_root: None,
    raw,
    edit: None,
  }
}
