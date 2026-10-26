use std::cell::Cell;

use serde_json::Value;
use toolu_protocol::exit::Exit;
use toolu_protocol::host::Host;
use toolu_runtime::cli::Outcome;

use super::guarded;
use crate::fast::HookRequest;

/// Codex's committed output schemas for the context events toolu registers.
const SCHEMAS: [(&str, &str); 3] = [
  (
    "SessionStart",
    include_str!(
      "../../../../../fixtures/codex-hook-schemas/session-start.command.output.schema.json"
    ),
  ),
  (
    "UserPromptSubmit",
    include_str!(
      "../../../../../fixtures/codex-hook-schemas/user-prompt-submit.command.output.schema.json"
    ),
  ),
  (
    "PostToolUse",
    include_str!(
      "../../../../../fixtures/codex-hook-schemas/post-tool-use.command.output.schema.json"
    ),
  ),
];

fn request(plugin: &str, name: &str, event: &str) -> HookRequest {
  HookRequest {
    plugin: plugin.to_owned(),
    name: name.to_owned(),
    event: Some(event.to_owned()),
    plugin_root: None,
  }
}

fn on(
  host: Host,
  request: &HookRequest,
  enforcing: bool,
  hook: impl FnOnce() -> Outcome,
) -> Outcome {
  guarded(request, enforcing, || host, hook)
}

fn message(outcome: &Outcome) -> String {
  let stdout = outcome.stdout.as_deref().unwrap_or_default();
  let json: Value = serde_json::from_str(stdout).unwrap_or_default();
  json["systemMessage"]
    .as_str()
    .unwrap_or_default()
    .to_owned()
}

/// Keys `schema` forbids or types it rejects in `payload` (Codex sets `additionalProperties: false`).
fn schema_errors(schema: &str, payload: &Value) -> Vec<String> {
  let schema: Value = serde_json::from_str(schema).unwrap_or_default();
  let mut errors = Vec::new();
  if schema["additionalProperties"] != Value::Bool(false) {
    errors.push("the schema no longer forbids additional properties".to_owned());
  }
  for (key, value) in payload.as_object().into_iter().flatten() {
    match schema["properties"][key]["type"].as_str() {
      Some("string") if value.is_string() => {}
      kind => errors.push(format!("{key}: schema type {kind:?}")),
    }
  }
  errors
}

#[test]
fn a_context_panic_is_a_schema_valid_system_message_naming_the_hook() {
  for (event, schema) in SCHEMAS {
    let request = request("jev", "probe", event);
    let outcome = on(Host::Codex, &request, false, || panic!("boom at line 3"));
    assert_eq!((outcome.exit, &outcome.stderr), (Exit::Success, &None));
    assert_eq!(
      message(&outcome),
      format!(
        "jev plugin: toolu jev hook probe on {event} panicked: boom at line 3. See docs/install.md."
      )
    );
    let payload: Value =
      serde_json::from_str(outcome.stdout.as_deref().unwrap_or_default()).unwrap();
    assert_eq!(
      schema_errors(schema, &payload),
      Vec::<String>::new(),
      "{event}"
    );
  }
}

#[test]
fn a_pre_compact_failure_is_named_too() {
  let request = request("toolu", "pre-compact", "PreCompact");
  let failed = || Outcome::failed(Exit::Failure, "  disk full\n".to_owned());
  let outcome = on(Host::Claude, &request, false, failed);
  assert_eq!(outcome.exit, Exit::Success);
  assert_eq!(
    message(&outcome),
    "toolu plugin: toolu hook pre-compact on PreCompact failed with exit 1: disk full. See docs/install.md."
  );
}

#[test]
fn exit_2_passes_only_where_the_host_gives_it_meaning() {
  let blocked = || Outcome {
    exit: Exit::Blocked,
    stdout: Some("{}".to_owned()),
    stderr: Some("feedback".to_owned()),
  };
  for event in ["PostToolUse", "UserPromptSubmit"] {
    let request = request("toolu", "post-tools", event);
    assert_eq!(
      on(Host::Codex, &request, false, blocked),
      blocked(),
      "{event}"
    );
  }
  let request = request("toolu", "session-start", "SessionStart");
  let outcome = on(Host::Codex, &request, false, blocked);
  assert_eq!(outcome.exit, Exit::Success);
  assert_eq!(
    message(&outcome),
    "toolu plugin: toolu hook session-start on SessionStart failed with exit 2: feedback. See docs/install.md."
  );
}

#[test]
fn cursor_gets_the_named_failure_on_stderr_with_exit_1() {
  let request = request("toolu", "session-start", "SessionStart");
  let outcome = on(Host::Cursor, &request, false, || panic!("boom"));
  assert_eq!(
    outcome,
    Outcome::failed(
      Exit::Failure,
      "toolu plugin: toolu hook session-start on SessionStart panicked: boom. See docs/install.md."
        .to_owned()
    )
  );
}

#[test]
fn an_enforcing_panic_blocks_and_other_enforcing_exits_pass_through() {
  let request = request("toolu", "pre-tools", "PreToolUse");
  let outcome = on(Host::Claude, &request, true, || panic!("boom"));
  assert_eq!(
    outcome,
    Outcome::failed(
      Exit::Blocked,
      "blocked: toolu plugin: toolu hook pre-tools on PreToolUse panicked: boom. See docs/install.md."
        .to_owned()
    )
  );
  let failed = || Outcome::failed(Exit::TempFail, "retry".to_owned());
  assert_eq!(on(Host::Claude, &request, true, failed), failed());
}

#[test]
fn a_successful_hook_is_returned_unchanged_without_detecting_the_host() {
  let detected = Cell::new(false);
  let request = request("toolu", "session-start", "SessionStart");
  let done = || Outcome::data("{\"systemMessage\":\"Toolu is on!\"}".to_owned());
  let host = || {
    detected.set(true);
    Host::Claude
  };
  assert_eq!(guarded(&request, false, host, done), done());
  assert!(!detected.get());
}
