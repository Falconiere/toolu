use std::path::Path;

use serde_json::{Map, Value, json};
use toolu_protocol::decision::Decision;
use toolu_protocol::host::Host;
use toolu_protocol::normalized::{NormalizedEvent, Session, Tool};
use toolu_protocol::text::Text;
use toolu_runtime::env::Env;
use toolu_runtime::registry::rule::RuleContext;

use super::{GATE_STATUS, failed, failing_context};
use crate::gate::Gate;

fn text(value: &str) -> Text {
  Text::new(value).unwrap()
}

fn bash() -> NormalizedEvent {
  NormalizedEvent::ToolPost {
    session: Session {
      session_id: text("s"),
      cwd: text("/p"),
      project_root: text("/p"),
      worktree: text("/p"),
    },
    tool: Tool {
      call_id: text("c"),
      name: text("Bash"),
      input: Map::new(),
    },
    output: None,
  }
}

fn object(value: &Value) -> Map<String, Value> {
  value.as_object().cloned().unwrap_or_default()
}

/// Runs the gate over `raw` in `project`, returning its decision and warnings.
fn run(project: &Path, raw: &Map<String, Value>) -> (Result<Decision, String>, Vec<String>) {
  let env = Env::from_pairs([("HOME", project.join("home").to_string_lossy().into_owned())]);
  let ctx = RuleContext {
    host: Host::Claude,
    env: &env,
    config_root: &project.join("home/.claude"),
    project_root: project,
    cwd: Some(project),
    plugin_root: None,
    raw,
    edit: None,
  };
  let mut warnings = Vec::new();
  let decided = GATE_STATUS.run_warning(&bash(), &ctx, &mut warnings);
  (decided, warnings)
}

#[test]
fn only_a_numeric_non_zero_status_is_a_failure() {
  for status in ["1", "2", "127", "01", "10"] {
    assert!(failed(status), "{status}");
  }
  for status in ["", "0", "00", "null", "1.5", "-1", "x y", "[\n  1\n]"] {
    assert!(!failed(status), "{status}");
  }
}

#[test]
fn the_advisory_keeps_bashs_two_character_newline() {
  assert_eq!(
    failing_context("bun test", "1"),
    "Global quality gate failing. Fix all errors/warnings/tests before new tasks.\\nFailed: bun test (exit 1)"
  );
}

#[test]
fn a_failure_records_and_advises_and_run_matches_run_warning() {
  let dir = tempfile::tempdir().unwrap();
  let raw =
    object(&json!({"tool_input": {"command": "bun test"}, "tool_response": {"exit_code": 1}}));
  let (decided, warnings) = run(dir.path(), &raw);
  let message = failing_context("bun test", "1");
  assert_eq!(
    decided,
    Ok(Decision::Advisory {
      message: text(&message)
    })
  );
  assert_eq!(warnings, Vec::<String>::new());
  let gate =
    std::fs::read_to_string(dir.path().join(".claude/tmp/quality-gate-status.json")).unwrap();
  assert!(gate.contains("\"source\": \"gate-status-hook\""), "{gate}");
  assert!(
    gate.contains("Quality command failed: bun test (exit 1)"),
    "{gate}"
  );
}

#[test]
fn an_unrecognized_gate_document_is_replaced_with_a_warning() {
  let dir = tempfile::tempdir().unwrap();
  let gate = dir.path().join(".claude/tmp/quality-gate-status.json");
  std::fs::create_dir_all(gate.parent().unwrap()).unwrap();
  std::fs::write(&gate, "[]\n").unwrap();
  let raw =
    object(&json!({"tool_input": {"command": "bun test"}, "tool_response": {"exit_code": 1}}));
  let (decided, warnings) = run(dir.path(), &raw);
  assert!(matches!(decided, Ok(Decision::Advisory { .. })));
  assert_eq!(warnings.len(), 1, "{warnings:?}");
  assert!(
    warnings[0].starts_with("gate-file: unrecognized gate file at "),
    "{warnings:?}"
  );
}

#[test]
fn an_unwritable_state_root_is_a_failure_of_the_gate() {
  let dir = tempfile::tempdir().unwrap();
  std::fs::write(dir.path().join(".claude"), "a file").unwrap();
  let raw = object(&json!({"tool_input": {"command": "ls"}}));
  let (decided, _) = run(dir.path(), &raw);
  assert!(decided.unwrap_err().starts_with("gate-status: "));
}

#[test]
fn a_first_pass_that_cannot_be_written_is_a_failure_of_the_gate() {
  let dir = tempfile::tempdir().unwrap();
  let gate = dir.path().join(".claude/tmp/quality-gate-status.json");
  std::fs::create_dir_all(gate.parent().unwrap()).unwrap();
  std::os::unix::fs::symlink(dir.path().join("missing/gate.json"), &gate).unwrap();
  let raw =
    object(&json!({"tool_input": {"command": "cargo test"}, "tool_response": {"exit_code": 0}}));
  let (decided, _) = run(dir.path(), &raw);
  assert!(decided.unwrap_err().starts_with("gate-status: "));
}
