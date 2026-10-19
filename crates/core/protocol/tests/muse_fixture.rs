//! The Muse hook contract (#544): every case of `fixtures/host/muse.json` leaves
//! `run_hook_io` with its exact stdout, stderr and exit, and the verdict a live
//! `muse plugins hook test` (Muse Code 1.4.10) gave for those bytes fits them.

#[path = "helpers/repo.rs"]
mod repo;

use std::process::ExitCode;

use repo::Res;
use serde_json::{Value, json};
use toolu_protocol::decision::{Decision, GateClass};
use toolu_protocol::encode::degrade_ask;
use toolu_protocol::event::HostEvent;
use toolu_protocol::exit::Exit;
use toolu_protocol::hook::{HookError, Io, Raw, Reply, run_hook_io};
use toolu_protocol::host::Host;

const FIXTURE: &str = "fixtures/host/muse.json";

/// One fixture case, parsed.
struct Case {
  name: String,
  event: HostEvent,
  reply: Value,
  class: Option<GateClass>,
  expect: Value,
  muse: Value,
}

fn field<'a>(case: &'a Value, key: &str) -> Res<&'a str> {
  let value = case.get(key).and_then(Value::as_str);
  value.ok_or_else(|| format!("no {key} in {case}"))
}

fn parse(case: &Value) -> Res<Case> {
  let class = match case.get("gateClass").and_then(Value::as_str) {
    None => None,
    Some("guardrail") => Some(GateClass::Guardrail),
    Some("judgement") => Some(GateClass::Judgement),
    Some(other) => return Err(format!("unknown gate class {other}")),
  };
  let part = |key: &str| case.get(key).cloned().ok_or(format!("no {key} in {case}"));
  Ok(Case {
    name: field(case, "name")?.to_owned(),
    event: HostEvent::from_slug(field(case, "event")?).ok_or("unknown event")?,
    reply: part("reply")?,
    class,
    expect: part("expect")?,
    muse: part("muse")?,
  })
}

fn every_case() -> Res<Vec<Case>> {
  repo::cases(FIXTURE)?.iter().map(parse).collect()
}

fn text(value: &Value, key: &str) -> Res<String> {
  let text = value.get(key).and_then(Value::as_str);
  text
    .map(str::to_owned)
    .ok_or(format!("no {key} in {value}"))
}

/// What the case's hook returns; `None` for a `panic` reply, which the test
/// raises itself.
fn reply(case: &Case) -> Res<Option<Result<Reply, HookError>>> {
  let reply = &case.reply;
  if let Some(decision) = reply.get("decision") {
    let decision: Decision =
      serde_json::from_value(decision.clone()).map_err(|err| err.to_string())?;
    let decision = match case.class {
      Some(class) => degrade_ask(Host::Muse, case.event, decision, class),
      None => decision,
    };
    return Ok(Some(Ok(Reply::from(decision))));
  }
  if reply.get("panic").is_some() {
    return Ok(None);
  }
  if let Some(message) = reply.get("error").and_then(Value::as_str) {
    return Ok(Some(Err(HookError::new(message))));
  }
  let raw = reply.get("raw").ok_or(format!("no reply in {reply}"))?;
  Ok(Some(Ok(Reply::Raw(Raw {
    stdout: text(raw, "stdout")?,
    stderr: text(raw, "stderr")?,
    exit: if raw.get("exit") == Some(&json!(0)) {
      Exit::Success
    } else {
      Exit::Blocked
    },
  }))))
}

/// `hook` run through `run_hook_io` for the case's event on Muse, in the
/// fixture's `expect` shape.
fn ran<F>(case: &Case, hook: F) -> Value
where
  F: FnOnce(&str) -> Result<Reply, HookError>,
{
  let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
  let io = Io {
    stdin: &b"{}"[..],
    stdout: &mut stdout,
    stderr: &mut stderr,
  };
  let code = run_hook_io(io, case.event, Host::Muse, hook);
  json!({
    "stdout": String::from_utf8_lossy(&stdout),
    "stderr": String::from_utf8_lossy(&stderr),
    "exit": if code == ExitCode::from(2) { 2 } else { 0 },
  })
}

#[test]
fn every_case_leaves_run_hook_with_its_exact_bytes() {
  let mut failures = Vec::new();
  for case in every_case().unwrap() {
    let got = match reply(&case).unwrap() {
      Some(reply) => ran(&case, |_| reply),
      None => ran(&case, |_| {
        panic!("{}", case.reply["panic"].as_str().unwrap())
      }),
    };
    if got != case.expect {
      failures.push(format!("{}: {got} != {}", case.name, case.expect));
    }
  }
  assert_eq!(failures, Vec::<String>::new());
}

#[test]
fn the_fixture_covers_every_event_decision_and_failure() {
  let cases = every_case().unwrap();
  let count = |key: &str| {
    cases
      .iter()
      .filter(|case| case.reply.get(key).is_some())
      .count()
  };
  assert_eq!(cases.len(), 98);
  assert_eq!(count("decision"), 64);
  assert_eq!((count("panic"), count("error"), count("raw")), (8, 8, 18));
  for event in HostEvent::ALL {
    let decisions = cases
      .iter()
      .filter(|case| case.event == event && case.reply.get("decision").is_some());
    assert_eq!(decisions.count(), 8, "{event:?}");
  }
}

#[test]
fn no_muse_output_carries_a_claude_shape() {
  for case in every_case().unwrap() {
    let stdout = text(&case.expect, "stdout").unwrap();
    for shape in [
      "hookSpecificOutput",
      "additionalContext",
      "permissionDecision",
    ] {
      assert!(!stdout.contains(shape), "{}: {stdout}", case.name);
    }
  }
}

/// What Muse should make of `expect`: `blocked` with a reason, `completed` with
/// a system message or nothing, or `failed` for a `SessionEnd` failure.
/// An exit 2 always carries an empty stdout and a non-blank stderr (AC-7).
fn intended(event: HostEvent, expect: &Value) -> Res<Value> {
  let (stdout, stderr) = (text(expect, "stdout")?, text(expect, "stderr")?);
  if expect.get("exit") == Some(&json!(2)) {
    if !stdout.is_empty() || stderr.trim().is_empty() {
      return Err(format!("exit 2 Muse ignores: {expect}"));
    }
    if event == HostEvent::SessionUnload {
      return Ok(json!({ "status": "failed" }));
    }
    let reason = stderr.trim();
    return Ok(json!({ "status": "blocked", "should_block": true, "block_reason": reason }));
  }
  if stdout.is_empty() {
    return Ok(json!({ "status": "completed" }));
  }
  let doc: Value = serde_json::from_str(&stdout).map_err(|err| err.to_string())?;
  Ok(match doc.get("systemMessage") {
    Some(message) => json!({ "status": "completed", "system_message": message }),
    None => json!({ "status": "blocked", "should_block": true, "block_reason": doc.get("reason") }),
  })
}

#[test]
fn every_live_muse_verdict_matches_the_intent_of_its_bytes() {
  for case in every_case().unwrap() {
    let want = intended(case.event, &case.expect).unwrap();
    assert_eq!(case.muse, want, "{}", case.name);
  }
}

/// The named case's expected output and Muse verdict.
fn named(name: &str) -> Res<(Value, Value)> {
  let case = every_case()?.into_iter().find(|case| case.name == name);
  let case = case.ok_or(format!("no case {name}"))?;
  Ok((case.expect, case.muse))
}

#[test]
fn each_row_of_the_epic_contract_has_its_case() {
  let block = "{\"decision\":\"block\",\"reason\":\"no .env writes\"}\n";
  for name in [
    "tool/pre: deny",
    "permission/evaluate: deny",
    "prompt: deny",
  ] {
    let (expect, muse) = named(name).unwrap();
    assert_eq!(expect, json!({ "stdout": block, "stderr": "", "exit": 0 }));
    assert_eq!(muse["should_block"], true, "{name}");
  }
  let (expect, muse) = named("tool/pre: ask from a guardrail gate").unwrap();
  assert_eq!(
    expect["stdout"],
    "{\"decision\":\"block\",\"reason\":\"confirm .env write\"}\n"
  );
  assert_eq!(muse["status"], "blocked");
  let (expect, muse) = named("tool/pre: hook panic").unwrap();
  let stderr = "blocked: toolu PreToolUse hook panicked: boom\n";
  assert_eq!(expect, json!({ "stdout": "", "stderr": stderr, "exit": 2 }));
  assert_eq!(muse["status"], "blocked");
  let (expect, muse) = named("session/start: advisory").unwrap();
  let advice = "{\"systemMessage\":\"prefer rg\"}\n";
  assert_eq!(expect, json!({ "stdout": advice, "stderr": "", "exit": 0 }));
  assert_eq!(muse["system_message"], "prefer rg");
  for name in [
    "tool/pre: raw exit 2 with blank streams",
    "tool/pre: raw exit 2 with JSON stdout",
  ] {
    let (expect, muse) = named(name).unwrap();
    assert_eq!(expect["stdout"], "", "{name}");
    assert_eq!(muse["status"], "blocked", "{name}");
  }
}
