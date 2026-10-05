//! No `toolu` and no Bun: enforcing events block, context events advise (AC-3).

use serde_json::Value;

use crate::sandbox::{Res, STARTUP, Sandbox, system_message};

const INSTALLER: &str = "curl -fsSL https://get.toolu.sh/pkg/toolu/install | bash";
const BREW: &str = "brew install falconiere/tap/toolu";
const SCHEMA: &str = include_str!(
  "../../../../tooling/fixtures/codex-hook-schemas/session-start.command.output.schema.json"
);

#[test]
fn pre_tool_use_blocks_naming_both_install_commands() {
  let run = Sandbox::new()
    .unwrap()
    .launch("PreToolUse", "pre-tools", "{}", &[])
    .unwrap();
  assert_eq!(run.code, 2, "{run:?}");
  assert_eq!(run.stdout, "");
  assert!(
    run
      .stderr
      .starts_with("blocked: toolu plugin: toolu is not installed")
  );
  assert!(run.stderr.contains(INSTALLER) && run.stderr.contains(BREW));
}

#[test]
fn session_start_prints_one_schema_valid_system_message() {
  let run = Sandbox::new()
    .unwrap()
    .launch("SessionStart", "session-start", STARTUP, &[])
    .unwrap();
  assert_eq!(run.code, 0, "{run:?}");
  assert_eq!(run.stderr, "");
  let message = system_message(&run).unwrap();
  assert!(message.starts_with("toolu plugin: toolu is not installed"));
  assert!(message.contains(INSTALLER) && message.contains(BREW));
  let payload: Value = serde_json::from_str(&run.stdout).unwrap();
  assert_eq!(schema_errors(&payload).unwrap(), Vec::<String>::new());
}

/// The keyword subset Codex's session-start schema uses on the top level:
/// `additionalProperties: false`, per-property `type`, and `required`.
fn schema_errors(payload: &Value) -> Res<Vec<String>> {
  let schema: Value = serde_json::from_str(SCHEMA)?;
  if schema.get("additionalProperties") != Some(&Value::Bool(false)) {
    return Err("the schema no longer forbids additional properties".into());
  }
  let properties = schema
    .get("properties")
    .and_then(Value::as_object)
    .ok_or("no properties")?;
  let object = payload.as_object().ok_or("the payload is not an object")?;
  let mut errors = Vec::new();
  for (key, value) in object {
    match properties.get(key) {
      None => errors.push(format!("{key}: not allowed")),
      Some(property) if !type_matches(property.get("type"), value) => {
        errors.push(format!("{key}: expected {:?}", property.get("type")));
      }
      Some(_) => {}
    }
  }
  let required = schema.get("required").and_then(Value::as_array);
  for key in required.into_iter().flatten() {
    if !object.contains_key(key.as_str().ok_or("a required key is not a string")?) {
      errors.push(format!("{key}: required"));
    }
  }
  Ok(errors)
}

fn type_matches(expected: Option<&Value>, value: &Value) -> bool {
  let actual = match value {
    Value::Null => "null",
    Value::Bool(_) => "boolean",
    Value::Number(_) => "number",
    Value::String(_) => "string",
    Value::Array(_) => "array",
    Value::Object(_) => "object",
  };
  match expected {
    Some(Value::String(name)) => name == actual,
    Some(Value::Array(names)) => names.iter().any(|name| name == actual),
    Some(Value::Null | Value::Bool(_) | Value::Number(_) | Value::Object(_)) | None => false,
  }
}
