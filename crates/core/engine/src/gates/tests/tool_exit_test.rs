use serde_json::{Map, Value, json};

use super::{tool_command, tool_exit_status, tool_interrupted};

/// What the gates read: command, exit status and the interrupt flag as text.
fn read(payload: &Value) -> (String, String, &'static str) {
  let empty = Map::new();
  let raw = payload.as_object().unwrap_or(&empty);
  let interrupted = if tool_interrupted(raw) {
    "true"
  } else {
    "false"
  };
  (tool_command(raw), tool_exit_status(raw), interrupted)
}

/// `{ tool_input: { command: "bun test" }, ...rest }`.
fn cmd(rest: &Value) -> Value {
  let mut payload = json!({ "tool_input": { "command": "bun test" } });
  if let (Some(into), Some(from)) = (payload.as_object_mut(), rest.as_object()) {
    into.extend(from.clone());
  }
  payload
}

/// `bun test` with `rest`, read as (exit status, interrupted).
fn check(cases: &[(&str, Value, &str, &str)]) {
  for (name, rest, exit, interrupted) in cases {
    let want = ("bun test".to_owned(), (*exit).to_owned(), *interrupted);
    assert_eq!(read(&cmd(rest)), want, "{name}");
  }
}

#[test]
fn the_exit_status_comes_from_metadata_then_the_response_then_the_output() {
  check(&[
    (
      "Claude metadata exit 1",
      json!({"tool_response": {"metadata": {"exit_code": 1}}}),
      "1",
      "false",
    ),
    (
      "Claude metadata exit 0",
      json!({"tool_response": {"metadata": {"exit_code": 0}}}),
      "0",
      "false",
    ),
    (
      "tool_response.exit_code",
      json!({"tool_response": {"exit_code": 3}}),
      "3",
      "false",
    ),
    (
      "Codex response",
      json!({"session_id": "s", "tool_name": "Bash", "tool_response": {"metadata": {"exit_code": 1}, "stdout": "", "stderr": "failed"}}),
      "1",
      "false",
    ),
    (
      "metadata false falls through",
      json!({"tool_response": {"metadata": {"exit_code": false}, "exit_code": 2}}),
      "2",
      "false",
    ),
    (
      "tool_output.exit_code",
      json!({"tool_output": {"exit_code": 4}}),
      "4",
      "false",
    ),
    (
      "no exit code at all",
      json!({"tool_response": {"stdout": ""}}),
      "",
      "false",
    ),
  ]);
}

#[test]
fn cursor_output_strings_and_jq_errors_read_as_typescript_reads_them() {
  check(&[
    (
      "Cursor tool_output JSON string",
      json!({"tool_output": "{\"exitCode\":1}"}),
      "1",
      "false",
    ),
    (
      "Cursor tool_output object exitCode",
      json!({"tool_output": {"exitCode": 0}}),
      "0",
      "false",
    ),
    (
      "tool_output non-JSON string",
      json!({"tool_output": "plain text"}),
      "",
      "false",
    ),
    (
      "string tool_response is a jq error",
      json!({"tool_response": "done", "tool_output": {"exitCode": 5}}),
      "5",
      "false",
    ),
    (
      "string null with tool_output",
      json!({"tool_response": {"exit_code": "null"}, "tool_output": "{\"exit_code\":0}"}),
      "0",
      "false",
    ),
    (
      "a tool_output number raises",
      json!({"tool_output": "5"}),
      "",
      "false",
    ),
    (
      "a tool_output null",
      json!({"tool_output": "null"}),
      "",
      "false",
    ),
  ]);
}

#[test]
fn odd_statuses_print_as_jq_prints_them() {
  check(&[
    (
      "string exit_code",
      json!({"tool_response": {"exit_code": "x y"}}),
      "x y",
      "false",
    ),
    (
      "string null exit_code",
      json!({"tool_response": {"exit_code": "null"}}),
      "null",
      "false",
    ),
    (
      "fractional exit_code",
      json!({"tool_response": {"exit_code": 1.5}}),
      "1.5",
      "false",
    ),
    (
      "array exit_code",
      json!({"tool_response": {"exit_code": [1]}}),
      "[\n  1\n]",
      "false",
    ),
    (
      "an object with DEL",
      json!({"tool_response": {"exit_code": {"k": "\u{7f}"}}}),
      "{\n  \"k\": \"\\u007f\"\n}",
      "false",
    ),
  ]);
  let float_one = serde_json::from_str::<Value>(r#"{"tool_response":{"exit_code":1.0}}"#).unwrap();
  assert_eq!(read(&float_one).1, "1", "JSON.parse reads 1.0 as 1");
}

#[test]
fn interrupted_is_true_only_when_it_prints_true() {
  check(&[
    (
      "interrupted true",
      json!({"tool_response": {"interrupted": true}}),
      "",
      "true",
    ),
    (
      "interrupted string true",
      json!({"tool_response": {"interrupted": "true"}}),
      "",
      "true",
    ),
    (
      "interrupted false",
      json!({"tool_response": {"interrupted": false, "exit_code": 0}}),
      "0",
      "false",
    ),
    (
      "a string response raises",
      json!({"tool_response": "done"}),
      "",
      "false",
    ),
  ]);
}

#[test]
fn the_command_is_printed_raw_or_empty() {
  let cases = [
    (json!({"tool_input": {"command": 7}}), "7", ""),
    (json!({"tool_response": {"exit_code": 0}}), "", "0"),
    (
      json!({"tool_input": "ls", "tool_response": {"exit_code": 0}}),
      "",
      "0",
    ),
    (
      json!({"tool_input": {"command": "bun test\n\n"}}),
      "bun test",
      "",
    ),
  ];
  for (payload, command, exit) in cases {
    assert_eq!(
      read(&payload),
      (command.to_owned(), exit.to_owned(), "false"),
      "{payload}"
    );
  }
  for payload in [Value::Null, json!([1]), json!("not json")] {
    assert_eq!(read(&payload), (String::new(), String::new(), "false"));
  }
}
