//! Every payload fixture under `fixtures/` deserializes for its host and
//! normalizes to the event TypeScript implies (AC-1). Descriptors render as the
//! TypeScript harness renders them (`helpers/render.rs`).

#[path = "helpers/render.rs"]
mod render;
#[path = "helpers/repo.rs"]
mod repo;

use repo::Res;
use serde_json::{Value, json};
use toolu_protocol::event::{EventKind, HostEvent};
use toolu_protocol::host::Host;
use toolu_protocol::normalize::Roots;
use toolu_protocol::normalized::NormalizedEvent;
use toolu_protocol::payload::{Payload, parse};
use toolu_protocol::text::Text;

const GATES: [&str; 5] = [
  "fixtures/gates/pre-tool-modules-a.json",
  "fixtures/gates/pre-tool-modules-b.json",
  "fixtures/gates/pre-tool-modules-c.json",
  "fixtures/gates/pretool-corpus.json",
  "fixtures/gates/posttool-corpus.json",
];

fn roots() -> Res<Roots> {
  Ok(Roots {
    project_root: Text::new(render::PROJECT)?,
    worktree: Text::new(render::PROJECT)?,
  })
}

fn normalized(host: Host, event: HostEvent, payload: &Value) -> Res<NormalizedEvent> {
  let parsed = parse(host, &payload.to_string())?;
  parsed
    .normalize(event, &roots()?)
    .ok_or_else(|| format!("{host:?} did not normalize {payload}").into())
}

/// The kind TypeScript's `toolEvent` gives a descriptor.
fn expected_kind(descriptor: &Value) -> EventKind {
  if descriptor.get("event") == Some(&json!("PostToolUse")) {
    return EventKind::ToolPost;
  }
  let name = descriptor.get("toolName").and_then(Value::as_str);
  let command = descriptor
    .pointer("/toolInput/command")
    .and_then(Value::as_str);
  let shell = matches!(name, Some("Bash" | "Shell"));
  if shell && command.is_some_and(|command| !command.is_empty()) {
    EventKind::ShellPre
  } else {
    EventKind::ToolPre
  }
}

fn hosts_of(case: &Value) -> Vec<Host> {
  match case
    .get("host")
    .and_then(Value::as_str)
    .and_then(Host::parse)
  {
    Some(host) => vec![host],
    None => vec![Host::Claude, Host::Codex],
  }
}

/// Render `descriptor` for each host (and Cursor before a tool) and check it.
fn check_descriptor(descriptor: &Value, hosts: &[Host]) -> Res<usize> {
  let descriptor = render::expand(descriptor);
  let event = match descriptor.get("event").and_then(Value::as_str) {
    Some("PostToolUse") => HostEvent::ToolPost,
    _ => HostEvent::ToolPre,
  };
  let mut targets = hosts.to_vec();
  if event == HostEvent::ToolPre {
    targets.push(Host::Cursor);
  }
  for host in &targets {
    let stdin = render::stdin(*host, &descriptor, render::PROJECT)?;
    let kind = normalized(*host, event, &stdin)?.kind();
    if kind != expected_kind(&descriptor) {
      return Err(format!("{host:?} {descriptor}: {kind:?}").into());
    }
  }
  Ok(targets.len())
}

/// Literal stdin: JSON objects parse for every host, anything else is an error.
fn check_stdin(text: &str, hosts: &[Host]) -> Res<bool> {
  let object = serde_json::from_str::<Value>(text).is_ok_and(|value| value.is_object());
  for host in hosts {
    if parse(*host, text).is_ok() != object {
      return Err(format!("{host:?}: {text:?}").into());
    }
  }
  Ok(object)
}

/// Payloads checked: (JSON stdin, non-JSON stdin, rendered descriptors).
fn check_gates(file: &str) -> Res<(usize, usize, usize)> {
  let mut counts = (0, 0, 0);
  for case in repo::cases(file)? {
    let hosts = hosts_of(&case);
    if let Some(stdin) = case.get("stdin").and_then(Value::as_str) {
      if check_stdin(stdin, &hosts)? {
        counts.0 += 1;
      } else {
        counts.1 += 1;
      }
      continue;
    }
    let descriptor = match case.get("fixture") {
      Some(fixture) => fixture.clone(),
      None => render::bash(case.get("command").and_then(Value::as_str).unwrap_or("")),
    };
    counts.2 += check_descriptor(&descriptor, &hosts)?;
  }
  Ok(counts)
}

#[test]
fn every_gate_case_parses_and_normalizes() {
  let counts: Vec<(usize, usize, usize)> = GATES
    .iter()
    .map(|file| check_gates(file).unwrap())
    .collect();
  assert_eq!(
    counts,
    [
      (0, 2, 214),
      (0, 0, 220),
      (0, 1, 358),
      (0, 2, 93),
      (1, 2, 26)
    ]
  );
}

/// The normalized kind of every lifecycle stdin that is a JSON object, by case
/// name, and the number of stdin texts that are not.
fn lifecycle_kinds() -> Res<(Vec<(String, EventKind)>, usize)> {
  let mut kinds = Vec::new();
  let mut errors = 0;
  for case in repo::cases("fixtures/gates/lifecycle.json")? {
    let stdin = case
      .get("stdin")
      .and_then(Value::as_str)
      .ok_or("no stdin")?;
    let host = hosts_of(&case).first().copied().ok_or("no host")?;
    if !check_stdin(stdin, &[host])? {
      errors += 1;
      continue;
    }
    let event = match case.get("hook").and_then(Value::as_str) {
      Some("user-prompt-submit") => HostEvent::Prompt,
      _ => HostEvent::SessionStart,
    };
    let kind = normalized(host, event, &serde_json::from_str(stdin)?)?.kind();
    let name = case.get("name").and_then(Value::as_str).ok_or("no name")?;
    kinds.push((name.to_owned(), kind));
  }
  Ok((kinds, errors))
}

#[test]
fn every_lifecycle_stdin_parses_and_maps_to_its_session_event() {
  let (kinds, errors) = lifecycle_kinds().unwrap();
  assert_eq!((kinds.len(), errors), (113, 4));
  let kind_of = |name: &str| {
    kinds
      .iter()
      .find(|(case, _)| case == name)
      .map(|(_, kind)| *kind)
  };
  let expected = [
    (
      "session-start: compact adds the post-compaction doc",
      EventKind::Compaction,
    ),
    ("session-start: resume", EventKind::SessionResume),
    ("session-start: clear", EventKind::SessionClear),
    (
      "session-start: a numeric source is an unknown event",
      EventKind::SessionStart,
    ),
    (
      "session-start: legacy session_event field",
      EventKind::SessionResume,
    ),
    (
      "session-start: a false source falls through like jq //",
      EventKind::SessionResume,
    ),
    ("user-prompt-submit: object prompt", EventKind::Prompt),
  ];
  for (name, kind) in expected {
    assert_eq!(kind_of(name), Some(kind), "{name}");
  }
}

#[test]
fn every_nudge_descriptor_and_savings_payload_parses() {
  let mut nudges = 0;
  for case in repo::cases("fixtures/ast-grep/nudge.json").unwrap() {
    let descriptor = json!({ "kind": "tool", "event": "PreToolUse",
      "toolName": case["toolName"], "toolInput": case["toolInput"] });
    nudges += check_descriptor(&descriptor, &[Host::Claude, Host::Codex]).unwrap();
  }
  let mut savings = 0;
  for case in repo::cases("fixtures/ast-grep/savings.json").unwrap() {
    let mut payload = render::expand(&case["payload"]);
    payload["hook_event_name"] = json!("PostToolUse");
    for host in [Host::Claude, Host::Codex] {
      let event = normalized(host, HostEvent::ToolPost, &payload).unwrap();
      assert_eq!(event.kind(), EventKind::ToolPost, "{payload}");
      savings += 1;
    }
  }
  assert_eq!((nudges, savings), (38 * 3, 35 * 2));
}

/// Every step of the quality case file `file`, rendered for Claude and Codex.
fn check_quality(file: &str) -> Res<usize> {
  let mut steps = 0;
  for case in repo::cases(file)? {
    let list = case
      .get("steps")
      .and_then(Value::as_array)
      .ok_or("no steps")?;
    for step in list {
      for host in [Host::Claude, Host::Codex] {
        steps += check_descriptor(&render::quality_step(step, host), &[host])?;
      }
    }
  }
  Ok(steps)
}

#[test]
fn every_quality_step_renders_as_a_post_tool_payload() {
  let steps: Vec<usize> = ["ts", "python", "rust"]
    .iter()
    .map(|name| check_quality(&format!("fixtures/quality/{name}.json")).unwrap())
    .collect();
  assert_eq!(steps, [127 * 2, 105 * 2, 129 * 2]);
}

#[test]
fn the_permission_evaluate_requests_are_claude_payloads() {
  let doc = repo::json("fixtures/opencode/permission-evaluate.json").unwrap();
  let mut requests = Vec::new();
  for case in doc["cases"].as_array().unwrap() {
    if let Some(request) = case.pointer("/expected/request") {
      requests.push(normalized(Host::Claude, HostEvent::ToolPre, request).unwrap());
    }
  }
  let names: Vec<&str> = requests
    .iter()
    .map(|event| event.tool().unwrap().name.as_str())
    .collect();
  assert_eq!(names, ["Edit", "Bash"]);
  assert_eq!(requests[1].kind(), EventKind::ShellPre);
  assert_eq!(requests[0].session().session_id.as_str(), "sess_1");
}

#[test]
fn the_protected_files_fixture_renders_as_its_conformance_suite_does() {
  let fixture = repo::json("fixtures/portable-core/protected-files-pre.json").unwrap();
  let payload = json!({ "tool_name": fixture["toolName"],
    "tool_input": { "file_path": format!("{}/.env", render::PROJECT) },
    "session_id": fixture["sessionId"], "tool_use_id": fixture["toolCallId"],
    "cwd": render::PROJECT });
  let event = normalized(Host::Claude, HostEvent::ToolPre, &payload).unwrap();
  let NormalizedEvent::ToolPre { session, tool } = &event else {
    panic!("not tool/pre: {event:?}");
  };
  assert_eq!(tool.name.as_str(), "Edit");
  assert_eq!(tool.call_id.as_str(), "call_fixture_1");
  assert_eq!(session.session_id.as_str(), "sess_fixture_1");
}

#[test]
fn the_pinned_host_opencode_captures_parse_as_input_and_output() {
  let text = repo::text("tools/toolu-opencode/contract/captures/tool-calls.jsonl").unwrap();
  let mut tools = Vec::new();
  for line in text.lines() {
    let capture: Value = serde_json::from_str(line).unwrap();
    let payload = json!({
      "input": { "tool": capture["tool"], "sessionID": capture["sessionID"], "callID": capture["callID"] },
      "output": { "args": capture["args"] },
    });
    let Payload::Opencode(parsed) = parse(Host::Opencode, &payload.to_string()).unwrap() else {
      panic!("not an OpenCode payload");
    };
    assert_eq!(parsed.input.call_id.as_str(), capture["callID"].as_str());
    assert_eq!(parsed.output, Some(json!({ "args": capture["args"] })));
    let event = Payload::Opencode(parsed.clone()).normalize(HostEvent::ToolPre, &roots().unwrap());
    assert_eq!(event, None);
    tools.push(parsed.input.tool.as_str().unwrap().to_owned());
  }
  assert_eq!(tools.len(), 10);
  assert!(tools.contains(&"bash".to_owned()) && tools.contains(&"apply_patch".to_owned()));
}
