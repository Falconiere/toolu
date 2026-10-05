use serde_json::{Map, Value, json};

use super::judge;
use crate::print_hook::entry_json;
use toolu_protocol::launcher::{Target, hook};

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
fn a_generated_entry_and_a_bun_entry_pass() {
  assert_eq!(
    judge("toolu", "PreToolUse", &native("PreToolUse", "pre-tools")),
    Vec::new()
  );
  let bun = json!({ "type": "command", "command": "exec bun hooks/dist/x.js" });
  assert_eq!(
    judge("toolu", "PreToolUse", bun.as_object().unwrap()),
    Vec::new()
  );
}

#[test]
fn a_native_entry_under_another_event_differs() {
  let problems = judge("toolu", "SessionStart", &native("PreToolUse", "pre-tools"));
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
    judge("toolu", "PreToolUse", &entry)[0]
      .0
      .starts_with("timeout must be")
  );
  entry.remove("timeout");
  assert!(
    judge("toolu", "PreToolUse", &entry)[0]
      .0
      .starts_with("timeout must be")
  );
  let mut typed = native("PreToolUse", "pre-tools");
  typed.insert("type".to_owned(), json!("prompt"));
  assert_eq!(
    judge("toolu", "PreToolUse", &typed)[0].0,
    "type must be \"command\""
  );
  let unnamed = json!({ "type": "command", "command": "toolu --hook-protocol", "timeout": 60 });
  assert_eq!(
    judge("toolu", "PreToolUse", unnamed.as_object().unwrap())[0].0,
    "native entry names no `hook <name>`"
  );
  let bad_event = native("PreToolUse", "pre-tools");
  assert!(
    judge("toolu", "pre tool", &bad_event)[0]
      .0
      .starts_with("event must match")
  );
}
