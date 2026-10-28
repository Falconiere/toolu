use serde_json::{Map, Value, json};

use super::{claude_input, claude_name};

fn object(value: &Value) -> Map<String, Value> {
  value.as_object().cloned().unwrap()
}

#[test]
fn opencode_tools_take_their_claude_names() {
  for (opencode, claude) in [
    ("shell", "Bash"),
    ("read", "Read"),
    ("write", "Write"),
    ("edit", "Edit"),
    ("patch", "apply_patch"),
    ("glob", "Glob"),
    ("grep", "Grep"),
    ("webfetch", "WebFetch"),
    ("websearch", "WebSearch"),
    ("skill", "Skill"),
    ("subagent", "Agent"),
    ("question", "AskUserQuestion"),
  ] {
    assert_eq!(claude_name(opencode), claude, "{opencode}");
  }
}

#[test]
fn unknown_and_mcp_tools_keep_their_names() {
  for name in ["context7_query-docs", "execute", "Bash", ""] {
    assert_eq!(claude_name(name), name);
  }
}

#[test]
fn an_edit_gains_claude_keys_beside_its_own() {
  let input =
    object(&json!({"path": "src/a.rs", "oldString": "a", "newString": "b", "replaceAll": true}));
  assert_eq!(
    Value::Object(claude_input("edit", &input)),
    json!({"path": "src/a.rs", "oldString": "a", "newString": "b", "replaceAll": true,
      "file_path": "src/a.rs", "old_string": "a", "new_string": "b", "replace_all": true})
  );
}

#[test]
fn a_patch_carries_its_text_as_apply_patch_command() {
  let input = object(&json!({"patchText": "*** Begin Patch\n*** End Patch"}));
  let out = claude_input("patch", &input);
  assert_eq!(out.get("command"), input.get("patchText"));
}

#[test]
fn a_key_already_present_is_never_overwritten_and_absent_keys_stay_absent() {
  let input = object(&json!({"path": "a", "file_path": "b"}));
  assert_eq!(
    claude_input("write", &input).get("file_path"),
    Some(&json!("b"))
  );
  let shell = claude_input("shell", &object(&json!({"command": "ls"})));
  assert_eq!(Value::Object(shell), json!({"command": "ls"}));
  let unknown = object(&json!({"path": "x"}));
  assert_eq!(claude_input("mystery", &unknown), unknown);
}
