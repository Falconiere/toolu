//! The Muse payload fixture (#543): every payload of `fixtures/host/muse-payload.json`,
//! captured from Muse Code 1.4.10 (#540) or built from its observed keys,
//! parses as `Payload::Muse` with its unknown keys in `rest`, and normalizes to
//! the exact `NormalizedEvent` wire in `expect`.

#[path = "helpers/repo.rs"]
mod repo;

use repo::Res;
use serde_json::Value;
use toolu_protocol::event::HostEvent;
use toolu_protocol::host::Host;
use toolu_protocol::normalize::Roots;
use toolu_protocol::payload::{Payload, parse};
use toolu_protocol::text::Text;

const FIXTURE: &str = "fixtures/host/muse-payload.json";

fn roots() -> Res<Roots> {
  Ok(Roots {
    project_root: Text::new("/project").map_err(|err| err.to_string())?,
    worktree: Text::new("/worktree").map_err(|err| err.to_string())?,
  })
}

/// `stdin` read as a Muse payload, and its sorted `rest` keys.
fn read(stdin: &Value) -> Res<(Payload, Value)> {
  let payload = parse(Host::Muse, &stdin.to_string()).map_err(|err| err.to_string())?;
  let Payload::Muse(hook) = &payload else {
    return Err(format!("not a Muse payload: {payload:?}"));
  };
  let mut keys: Vec<&String> = hook.rest.keys().collect();
  keys.sort_unstable();
  let keys = serde_json::to_value(keys).map_err(|err| err.to_string())?;
  Ok((payload, keys))
}

/// What one case normalizes to, as wire JSON; `Null` for a parse-only case.
fn check(case: &Value) -> Res<Value> {
  let name = case.get("name").and_then(Value::as_str).ok_or("no name")?;
  let stdin = case.get("stdin").ok_or("no stdin")?;
  let (payload, keys) = read(stdin)?;
  if Some(&keys) != case.get("rest") {
    return Err(format!("{name}: rest {keys}"));
  }
  let Some(slug) = case.get("event").and_then(Value::as_str) else {
    return Ok(Value::Null);
  };
  let event = HostEvent::from_slug(slug).ok_or("unknown event")?;
  let normalized = payload
    .normalize(event, &roots()?)
    .ok_or_else(|| format!("{name}: no event"))?;
  serde_json::to_value(normalized).map_err(|err| err.to_string())
}

#[test]
fn every_muse_payload_parses_keeps_its_rest_and_normalizes_to_its_wire() {
  let cases = repo::cases(FIXTURE).unwrap();
  let mut parse_only = 0;
  for case in &cases {
    let got = check(case).unwrap();
    let expect = case.get("expect").unwrap_or(&Value::Null);
    assert_eq!(&got, expect, "{}", case["name"]);
    parse_only += usize::from(expect.is_null());
  }
  assert_eq!((cases.len(), parse_only), (86, 5));
}

/// The case named `name`: its stdin and the wire it normalizes to.
fn named(name: &str) -> Res<(Value, Value)> {
  let cases = repo::cases(FIXTURE)?;
  let case = cases.iter().find(|case| case["name"] == name);
  let case = case.ok_or_else(|| format!("no case {name}"))?;
  Ok((case["stdin"].clone(), check(case)?))
}

fn field(name: &str, key: &str) -> Res<Value> {
  let wire = named(name)?.1;
  wire
    .get(key)
    .cloned()
    .ok_or_else(|| format!("{name}: no {key}"))
}

#[test]
fn muse_tool_calls_take_claude_names_and_keep_their_input_and_output() {
  assert_eq!(field("bash PreToolUse", "type").unwrap(), "shell/pre");
  assert_eq!(
    field("bash PreToolUse", "command").unwrap(),
    "read line; echo got:$line"
  );
  assert_eq!(
    field("bash PreToolUse without a command", "type").unwrap(),
    "tool/pre"
  );
  assert_eq!(
    field("bash PreToolUse without a command", "toolName").unwrap(),
    "Bash"
  );
  for (tool, claude) in [
    ("read_file", "Read"),
    ("edit_file", "Edit"),
    ("write_file", "Write"),
  ] {
    let (stdin, wire) = named(&format!("{tool} PreToolUse")).unwrap();
    assert_eq!(wire["toolName"], claude);
    assert_eq!(wire["toolInput"], stdin["tool_input"]);
  }
  assert_eq!(
    field("submit_reminder_decision PreToolUse", "toolName").unwrap(),
    "submit_reminder_decision"
  );
  assert!(field("bash PostToolUse", "toolOutput").unwrap().is_string());
  assert!(
    field("edit_file PostToolUse", "toolOutput")
      .unwrap()
      .is_object()
  );
}

#[test]
fn muse_permission_session_and_prompt_events_map_like_claude() {
  assert_eq!(
    field("bash PermissionRequest", "permission").unwrap(),
    "Bash"
  );
  let as_tool = "bash PermissionRequest read as a tool call has no call id";
  assert_eq!(field(as_tool, "toolCallId").unwrap(), "unknown");
  for (source, kind) in [
    ("startup", "session/start"),
    ("resume", "session/resume"),
    ("clear", "session/clear"),
    ("compact", "compaction"),
  ] {
    assert_eq!(
      field(&format!("SessionStart source {source}"), "type").unwrap(),
      kind
    );
  }
  assert_eq!(
    field("SessionStart without a source", "type").unwrap(),
    "session/start"
  );
  assert_eq!(
    field("UserPromptSubmit with a string prompt", "prompt").unwrap(),
    "list the files"
  );
}
