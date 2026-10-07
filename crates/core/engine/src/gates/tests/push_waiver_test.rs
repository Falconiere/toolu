use std::path::Path;

use serde_json::{Map, json};
use toolu_protocol::decision::Decision;
use toolu_protocol::host::Host;
use toolu_protocol::normalized::{NormalizedEvent, Session, Tool};
use toolu_protocol::text::Text;
use toolu_runtime::env::Env;
use toolu_runtime::registry::rule::RuleContext;

use super::{PUSH_WAIVER, push_failed};
use crate::gate::Gate;

fn text(value: &str) -> Text {
  Text::new(value).unwrap()
}

fn event(tool: &str) -> NormalizedEvent {
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
fn only_an_empty_null_or_zero_status_counts_as_landed() {
  for status in ["", "null", "0"] {
    assert!(!push_failed(status), "{status}");
  }
  for status in ["1", "128", "00", "x"] {
    assert!(push_failed(status), "{status}");
  }
}

#[test]
fn anything_but_a_landed_shell_push_is_allowed_without_touching_disk() {
  let dir = tempfile::tempdir().unwrap();
  let env = Env::from_pairs([("HOME", "/nonexistent")]);
  let cases = [
    ("Write", json!({"tool_input": {"command": "git push"}})),
    ("Bash", json!({"tool_input": {"command": "git status"}})),
    (
      "Bash",
      json!({"tool_input": {"command": "git push"}, "tool_response": {"exit_code": 1}}),
    ),
    (
      "Bash",
      json!({"tool_input": {"command": "git push"}, "tool_response": {"interrupted": true}}),
    ),
    ("Shell", json!({"tool_input": {"command": "git push"}})),
  ];
  for (tool, raw) in cases {
    let raw = raw.as_object().cloned().unwrap_or_default();
    let ctx = RuleContext {
      host: Host::Claude,
      env: &env,
      config_root: Path::new("/nonexistent/.claude"),
      project_root: dir.path(),
      cwd: None,
      raw: &raw,
      edit: None,
    };
    assert_eq!(
      PUSH_WAIVER.run(&event(tool), &ctx),
      Ok(Decision::Allow),
      "{tool} {raw:?}"
    );
  }
  assert_eq!(
    std::fs::read_dir(dir.path()).unwrap().count(),
    0,
    "no git, no state"
  );
}
