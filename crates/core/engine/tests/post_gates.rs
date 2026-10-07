//! gate-status through the post-tool walk on a real git project (the scenarios
//! of `gate-status.test.ts`): which commands record, which clear, which leave
//! the gate alone, and the exact advisory, state path and warning.

#[path = "helpers/hook.rs"]
mod hook;
#[path = "helpers/sandbox.rs"]
mod sandbox;

use hook::Hook;
use sandbox::{Res, write};
use serde_json::{Value, json};
use toolu_engine::Phase;
use toolu_engine::builtins::POST_TOOL;
use toolu_protocol::host::Host;

const GATE: &str = "project/.claude/tmp/quality-gate-status.json";

/// A Bash call that already ran with `response`.
fn bash(command: &str, response: &Value) -> String {
  json!({
    "session_id": "s1",
    "hook_event_name": "PostToolUse",
    "tool_name": "Bash",
    "tool_input": {"command": command},
    "tool_response": response,
  })
  .to_string()
}

fn exit(command: &str, code: i64) -> String {
  bash(command, &json!({"metadata": {"exit_code": code}}))
}

/// Dispatches each payload in order; the last result's stdout and stderr.
fn run(hook: &Hook, payloads: &[String]) -> (String, String) {
  let mut last = (String::new(), String::new());
  for stdin in payloads {
    let dispatched = hook.run(Phase::Post, stdin, POST_TOOL, &[]);
    assert_eq!(dispatched.result.exit_code, 0, "{stdin}");
    last = (dispatched.result.stdout, dispatched.result.stderr);
  }
  last
}

fn status(hook: &Hook, rel: &str) -> Option<String> {
  let text = std::fs::read_to_string(hook.sb.path(rel)).ok()?;
  let doc: Value = serde_json::from_str(&text).ok()?;
  doc.get("status").and_then(Value::as_str).map(str::to_owned)
}

fn seed(hook: &Hook, doc: &Value) -> Res<()> {
  let text = serde_json::to_string_pretty(doc).map_err(|err| err.to_string())?;
  write(&hook.sb.path(GATE), &format!("{text}\n"))
}

fn legacy(source: &str, status: &str) -> Value {
  json!({"status": status, "source": source, "reason": "seeded by test", "updatedAt": "2026-01-01T00:00:00Z"})
}

fn ts_entry() -> Value {
  let entry = json!({"source": "ts-quality-hook", "reason": "bad ts", "violations": "viol\n", "updatedAt": "2026-01-01T00:00:00Z"});
  json!({
    "status": "failing", "reason": "bad ts", "source": "ts-quality-hook", "file": "/p/a.ts",
    "violations": "viol\n", "entries": {"/p/a.ts": entry}, "updatedAt": "2026-01-01T00:00:00Z",
  })
}

/// The gate status `payloads` leave after `seeded` (`None`: no file).
fn left(seeded: Option<&Value>, payloads: &[String]) -> Res<Option<String>> {
  let hook = Hook::new(Host::Claude)?;
  if let Some(doc) = seeded {
    seed(&hook, doc)?;
  }
  run(&hook, payloads);
  Ok(status(&hook, GATE))
}

/// One scenario: seed, payloads, and the gate status left.
fn scenario(seeded: Option<&Value>, payloads: &[String], want: Option<&str>) {
  let got = left(seeded, payloads);
  assert_eq!(
    got.as_ref().map(Option::as_deref),
    Ok(want),
    "{seeded:?} {payloads:?}"
  );
}

#[test]
fn quality_commands_record_and_clear_the_command_channel() {
  scenario(None, &[exit("ls -la", 0)], None);
  scenario(None, &[exit("cargo clippy", 0)], Some("passing"));
  scenario(None, &[exit("bun test", 1)], Some("failing"));
  scenario(
    None,
    &[exit("bun test", 1), exit("cargo test", 0)],
    Some("passing"),
  );
  scenario(
    None,
    &[exit("tsc", 0), exit("bun test", 0)],
    Some("passing"),
  );
  scenario(
    Some(&legacy("rust-quality-hook", "failing")),
    &[exit("cargo clippy", 0)],
    Some("failing"),
  );
  scenario(
    Some(&legacy("ts-quality-hook", "failing")),
    &[exit("bun run check", 0)],
    Some("failing"),
  );
  scenario(
    Some(&legacy("ts-quality-hook", "passing")),
    &[exit("bun test", 1)],
    Some("failing"),
  );
  scenario(Some(&ts_entry()), &[exit("bun test", 1)], Some("failing"));
  scenario(Some(&ts_entry()), &[exit("cargo test", 0)], Some("failing"));
}

#[test]
fn triggers_pass_and_names_do_not() {
  for command in [
    "cargo test",
    "  cargo clippy",
    "cd crate && cargo test",
    "bun run check",
    "cd web && bun test",
    "find . | cargo test",
    "foo | tsc",
    "tsc --noEmit",
  ] {
    scenario(None, &[exit(command, 0)], Some("passing"));
  }
  for command in [
    "cat tsconfig.json",
    "ls tooling/foo/test.sh",
    "vitests-helper",
    "cattsc",
  ] {
    scenario(None, &[exit(command, 0)], None);
  }
}

#[test]
fn an_unreported_or_odd_status_records_nothing() {
  scenario(None, &[bash("bun test", &json!({}))], None);
  scenario(None, &[bash("bun test", &json!({"exit_code": 1.5}))], None);
  let edit = json!({"tool_name": "Write", "tool_input": {"file_path": "a.ts"}}).to_string();
  scenario(None, &[edit], None);
  let cursor = json!({"tool_name": "Shell", "tool_input": {"command": "bun test"}, "tool_output": "{\"exitCode\":1}"});
  scenario(None, &[cursor.to_string()], Some("failing"));
}

#[test]
fn an_unobservable_exit_never_vouches_for_a_pass() {
  // The issue's scenario: from nothing, a piped `cargo test` exiting 0 records no pass.
  scenario(None, &[exit("cargo test 2>&1 | tail -5", 0)], None);
  for command in [
    "echo \"remember to run bun test later\"",
    "bun test 2>&1 | tail -20",
    "cargo test 2>&1 | tail -5",
    "bun test 2>&1 | tail; bun run lint",
    "bun test || true",
    "bun test; echo done",
  ] {
    scenario(
      None,
      &[exit("bun test", 1), exit(command, 0)],
      Some("failing"),
    );
  }
  scenario(None, &[exit("bun test 2>&1 | tail -5", 1)], None);
  scenario(
    None,
    &[exit("bun run lint && bun test", 1)],
    Some("failing"),
  );
}

#[test]
fn the_failure_advisory_and_the_first_pass_are_typescripts_bytes() {
  let hook = Hook::new(Host::Claude).unwrap();
  let (stdout, stderr) = run(&hook, &[exit("bun test", 1)]);
  assert_eq!(
    stdout,
    "{\n  \"hookSpecificOutput\": {\n    \"hookEventName\": \"PostToolUse\",\n    \"additionalContext\": \"Global quality gate failing. Fix all errors/warnings/tests before new tasks.\\\\nFailed: bun test (exit 1)\"\n  }\n}\n"
  );
  assert_eq!(stderr, "");
  let gate: Value =
    serde_json::from_str(&std::fs::read_to_string(hook.sb.path(GATE)).unwrap()).unwrap();
  assert_eq!(gate["source"], "gate-status-hook");
  assert_eq!(gate["reason"], "Quality command failed: bun test (exit 1)");
  let fresh = Hook::new(Host::Claude).unwrap();
  run(&fresh, &[exit("cargo test", 0)]);
  let text = std::fs::read_to_string(fresh.sb.path(GATE)).unwrap();
  let (head, stamp) = text.split_at(text.find("\"updatedAt\": \"").unwrap() + 14);
  assert_eq!(
    head,
    "{\n  \"status\": \"passing\",\n  \"source\": \"cargo test\",\n  \"updatedAt\": \""
  );
  assert!(
    stamp.len() == 24 && stamp.ends_with("Z\"\n}\n"),
    "{stamp:?}"
  );
}

#[test]
fn codex_records_under_its_own_state_dir() {
  let hook = Hook::new(Host::Codex).unwrap();
  run(&hook, &[exit("bun test", 1)]);
  assert_eq!(
    status(&hook, "project/.codex/tmp/quality-gate-status.json").as_deref(),
    Some("failing")
  );
  assert!(!hook.sb.path(GATE).exists());
}

#[test]
fn an_unrecognized_gate_file_is_replaced_with_a_warning_on_stderr() {
  let hook = Hook::new(Host::Claude).unwrap();
  seed(&hook, &json!([])).unwrap();
  let (stdout, stderr) = run(&hook, &[exit("bun test", 1)]);
  assert!(stdout.contains("Failed: bun test (exit 1)"), "{stdout}");
  let gate = hook.sb.path(GATE);
  let prefix = format!("gate-file: unrecognized gate file at {} (", gate.display());
  assert!(
    stderr.starts_with(&prefix) && stderr.ends_with("); replacing it\n"),
    "{stderr}"
  );
  assert_eq!(stderr.lines().count(), 1, "{stderr}");
  assert_eq!(status(&hook, GATE).as_deref(), Some("failing"));
}

#[test]
fn a_registry_advisory_merges_after_the_built_ins() {
  let hook = Hook::new(Host::Claude).unwrap();
  let advise = r#"printf '%s\n' '{"hookSpecificOutput":{"hookEventName":"PostToolUse","additionalContext":"from the registry"}}'"#;
  hook.sh(Phase::Post, "x@t__advise.sh", advise).unwrap();
  let (stdout, _) = run(&hook, &[exit("bun test", 1)]);
  let gate = stdout.find("Failed: bun test (exit 1)").unwrap();
  assert!(
    stdout
      .find("from the registry")
      .is_some_and(|registry| registry > gate),
    "{stdout}"
  );
  assert!(hook.dir(Phase::Post).starts_with(hook.config_root()));
}
