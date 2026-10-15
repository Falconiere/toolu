use serde_json::{Map, Value, json};

use super::judge;
use crate::print_hook::entry_json;
use toolu_protocol::launcher::{Target, hook};

fn judge_here(
  plugin: &str,
  event: &str,
  entry: &Map<String, Value>,
) -> Vec<(String, Option<String>)> {
  judge(plugin, event, entry)
}

fn native(event: &str, name: &str) -> Map<String, Value> {
  let target = Target {
    plugin: "toolu",
    event,
    name,
  };
  let text = entry_json(&hook(&target, 60).unwrap());
  serde_json::from_str(&text).unwrap()
}

#[test]
fn a_generated_entry_passes_and_a_bun_entry_is_refused() {
  assert_eq!(
    judge_here("toolu", "PreToolUse", &native("PreToolUse", "pre-tools")),
    Vec::new()
  );
  let bun = json!({
    "type": "command",
    "command": "bun \"${CLAUDE_PLUGIN_ROOT}/hooks/dist/pre-tools.js\"",
    "commandWindows": "bun \"%PLUGIN_ROOT%\\hooks\\dist\\pre-tools.js\"",
  });
  let problems = judge_here("toolu", "PreToolUse", bun.as_object().unwrap());
  assert_eq!(problems.len(), 1, "{problems:?}");
  assert!(problems[0].0.starts_with("not the native launcher"));
}

#[test]
fn a_native_entry_under_another_event_differs() {
  let problems = judge_here("toolu", "SessionStart", &native("PreToolUse", "pre-tools"));
  assert_eq!(problems.len(), 2, "{problems:?}");
  assert!(problems[0].0.starts_with("command differs"));
  assert!(
    problems[0]
      .1
      .as_ref()
      .unwrap()
      .contains("--event SessionStart")
  );
}

#[test]
fn timeout_type_and_name_problems_are_named() {
  let mut entry = native("PreToolUse", "pre-tools");
  entry.insert("timeout".to_owned(), json!(601));
  assert!(
    judge_here("toolu", "PreToolUse", &entry)[0]
      .0
      .starts_with("timeout must be")
  );
  entry.remove("timeout");
  assert!(
    judge_here("toolu", "PreToolUse", &entry)[0]
      .0
      .starts_with("timeout must be")
  );
  let mut typed = native("PreToolUse", "pre-tools");
  typed.insert("type".to_owned(), json!("prompt"));
  assert_eq!(
    judge_here("toolu", "PreToolUse", &typed)[0].0,
    "type must be \"command\""
  );
  let unnamed = json!({ "type": "command", "command": "toolu --hook-protocol", "timeout": 60 });
  assert_eq!(
    judge_here("toolu", "PreToolUse", unnamed.as_object().unwrap())[0].0,
    "native entry names no `hook <name>`"
  );
  let bad_event = native("PreToolUse", "pre-tools");
  assert!(
    judge_here("toolu", "pre tool", &bad_event)[0]
      .0
      .starts_with("event must match")
  );
}

#[test]
fn a_hand_written_toolu_command_is_refused() {
  let dir = tempfile::tempdir().unwrap();
  let hooks = dir.path().join("plugins/toolu/hooks");
  std::fs::create_dir_all(&hooks).unwrap();
  std::fs::write(
    hooks.join("hooks.json"),
    r#"{"hooks":{"PreToolUse":[{"hooks":[{"type":"command","command":"toolu hook pre-tools"}]}]}}"#,
  )
  .unwrap();
  let found = super::check(dir.path(), "toolu");
  assert_eq!(found.len(), 1, "{found:?}");
  assert!(
    found[0].problem.starts_with("not the native launcher"),
    "{found:?}"
  );
}

#[test]
fn an_unreadable_hooks_json_is_a_finding_and_an_absent_one_is_not() {
  let dir = tempfile::tempdir().unwrap();
  assert_eq!(super::check(dir.path(), "x"), Vec::new());
  std::fs::create_dir_all(dir.path().join("plugins/x/hooks/hooks.json")).unwrap();
  let found = super::check(dir.path(), "x");
  assert_eq!(found.len(), 1, "{found:?}");
  assert!(found[0].problem.starts_with("cannot read: "), "{found:?}");
}
