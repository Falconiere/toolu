//! The shared encode fixture (AC-2, AC-3): the Rust encoder reproduces every case
//! of `fixtures/host/encode.json` byte for byte, as the TypeScript encoder does
//! (`packages/toolu-core/src/host/__tests__/host-encode-fixture.test.ts`), and
//! every Claude and Codex output validates against Codex's output schema.

#[path = "helpers/repo.rs"]
mod repo;

use repo::Res;
use serde_json::{Value, json};
use toolu_protocol::decision::{Decision, GateClass};
use toolu_protocol::encode::{Encoded, degrade_ask, encode};
use toolu_protocol::event::HostEvent;
use toolu_protocol::host::Host;

const FIXTURE: &str = "fixtures/host/encode.json";

/// One fixture case, parsed.
struct Case {
  name: String,
  host: Host,
  event: HostEvent,
  decision: Decision,
  expect: Value,
}

fn field<'a>(case: &'a Value, key: &str) -> Res<&'a str> {
  let value = case.get(key).and_then(Value::as_str);
  value.ok_or_else(|| format!("no {key} in {case}"))
}

fn parse(case: &Value) -> Res<Case> {
  let host = Host::parse(field(case, "host")?).ok_or("unknown host")?;
  let event = HostEvent::from_slug(field(case, "event")?).ok_or("unknown event")?;
  let decision = case.get("decision").cloned().ok_or("no decision")?;
  let decision: Decision = serde_json::from_value(decision).map_err(|err| err.to_string())?;
  let decision = match case.get("gateClass").and_then(Value::as_str) {
    None => decision,
    Some("guardrail") => degrade_ask(host, event, decision, GateClass::Guardrail),
    Some("judgement") => degrade_ask(host, event, decision, GateClass::Judgement),
    Some(other) => return Err(format!("unknown gate class {other}")),
  };
  Ok(Case {
    name: field(case, "name")?.to_owned(),
    host,
    event,
    decision,
    expect: case.get("expect").cloned().ok_or("no expect")?,
  })
}

fn every_case() -> Res<Vec<Case>> {
  repo::cases(FIXTURE)?.iter().map(parse).collect()
}

/// What the Rust encoder makes of `case`, in the fixture's `expect` shape.
fn encoded(case: &Case) -> Value {
  match encode(case.host, case.event, &case.decision) {
    Ok(Encoded::Command(stdout)) => json!({ "stdout": stdout }),
    Ok(Encoded::Callback(callback)) => json!({ "callback": callback.json() }),
    Err(err) => json!({ "error": err.to_string() }),
  }
}

#[test]
fn the_fixture_covers_every_host_event_and_decision() {
  let cases = every_case().unwrap();
  let count = |kind: &str| {
    cases
      .iter()
      .filter(|case| case.expect.get(kind).is_some())
      .count()
  };
  assert_eq!(cases.len(), 299);
  assert_eq!(count("stdout"), 238);
  assert_eq!(count("callback"), 57);
  assert_eq!(count("error"), 4);
}

#[test]
fn every_case_encodes_to_the_typescript_bytes() {
  let failures: Vec<String> = every_case()
    .unwrap()
    .iter()
    .filter(|case| encoded(case) != case.expect)
    .map(|case| format!("{}: {} != {}", case.name, encoded(case), case.expect))
    .collect();
  assert_eq!(failures, Vec::<String>::new());
}

#[test]
fn a_codex_judgement_ask_before_a_tool_is_advice_as_in_typescript() {
  let case = every_case()
    .unwrap()
    .into_iter()
    .find(|case| case.name == "codex tool/pre: ask from a judgement gate")
    .unwrap();
  let expected = "{\"hookSpecificOutput\":{\"hookEventName\":\"PreToolUse\",\
                  \"additionalContext\":\"confirm .env write\"}}\n";
  assert_eq!(case.expect, json!({ "stdout": expected }));
  assert_eq!(encoded(&case), case.expect);
}

/// The Codex schema file for `event`; `PreCompact` and `SessionEnd` have none.
fn schema_name(event: HostEvent) -> Option<&'static str> {
  let name = match event {
    HostEvent::ToolPre | HostEvent::ShellPre => "pre-tool-use",
    HostEvent::ToolPost => "post-tool-use",
    HostEvent::SessionStart => "session-start",
    HostEvent::Prompt => "user-prompt-submit",
    HostEvent::PermissionEvaluate => "permission-request",
    HostEvent::PreCompact | HostEvent::SessionUnload => return None,
  };
  Some(name)
}

fn schema(name: &str) -> Res<Value> {
  repo::json(&format!(
    "fixtures/codex-hook-schemas/{name}.command.output.schema.json"
  ))
}

#[test]
fn every_claude_and_codex_output_validates_against_the_codex_schema() {
  let mut validated: Vec<HostEvent> = Vec::new();
  let mut errors: Vec<String> = Vec::new();
  for case in every_case().unwrap() {
    let Some(Value::String(stdout)) = case.expect.get("stdout") else {
      continue;
    };
    let command_host = matches!(case.host, Host::Claude | Host::Codex);
    let Some(name) = schema_name(case.event).filter(|_| command_host && !stdout.is_empty()) else {
      continue;
    };
    let schema = schema(name).unwrap();
    let output: Value = serde_json::from_str(stdout.as_str()).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    errors.extend(
      validator
        .iter_errors(&output)
        .map(|err| format!("{}: {err}", case.name)),
    );
    validated.push(case.event);
  }
  assert_eq!(errors, Vec::<String>::new());
  for event in [
    HostEvent::ToolPre,
    HostEvent::ShellPre,
    HostEvent::ToolPost,
    HostEvent::SessionStart,
    HostEvent::Prompt,
    HostEvent::PermissionEvaluate,
  ] {
    assert!(
      validated.contains(&event),
      "no output validated for {event:?}"
    );
  }
}

#[test]
fn the_codex_schema_refuses_an_ask_where_it_would_be_wrong_shaped() {
  let schema = schema("post-tool-use").unwrap();
  let validator = jsonschema::validator_for(&schema).unwrap();
  let ask = json!({ "hookSpecificOutput": { "hookEventName": "PostToolUse",
    "permissionDecision": "ask" } });
  assert!(!validator.is_valid(&ask));
}
