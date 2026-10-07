//! Reading the hook payload (AC-15): `JSON.parse` accepts lone surrogate escapes
//! and any depth, serde neither. Surrogates read as U+FFFD so gates still see the
//! tool; nesting past serde's limit fails closed; the config switch comes first.

#[path = "helpers/hook.rs"]
mod hook;
#[path = "helpers/sandbox.rs"]
mod sandbox;

use std::sync::Mutex;

use hook::Hook;
use toolu_engine::Phase;
use toolu_engine::gate::Gate;
use toolu_protocol::decision::Decision;
use toolu_protocol::host::Host;
use toolu_protocol::normalized::NormalizedEvent;
use toolu_runtime::registry::rule::RuleContext;

/// A built-in that records each shell command it sees.
struct Seen(Mutex<Vec<String>>);

impl Gate for Seen {
  fn name(&self) -> &'static str {
    "seen"
  }

  fn run(&self, event: &NormalizedEvent, _ctx: &RuleContext<'_>) -> Result<Decision, String> {
    if let NormalizedEvent::ShellPre { command, .. } = event {
      self
        .0
        .lock()
        .map_err(|err| err.to_string())?
        .push(command.as_str().to_owned());
    }
    Ok(Decision::Allow)
  }
}

fn bash(command_json: &str) -> String {
  format!(r#"{{"tool_name":"Bash","tool_input":{{"command":"{command_json}"}},"session_id":"s1"}}"#)
}

/// A payload whose `tool_input` holds `depth` nested arrays.
fn nested(depth: usize) -> String {
  format!(
    r#"{{"tool_name":"Bash","tool_input":{{"command":"ls","x":{}{}}}}}"#,
    "[".repeat(depth),
    "]".repeat(depth)
  )
}

#[test]
fn lone_surrogates_read_as_replacement_characters_and_modules_get_the_raw_text() {
  let hook = Hook::new(Host::Claude).unwrap();
  hook
    .sh(
      Phase::Pre,
      "x@t__echo.sh",
      r#"jq -cn --arg i "$input" '{systemMessage: $i}'"#,
    )
    .unwrap();
  let seen = Seen(Mutex::new(Vec::new()));
  let cases = [
    (r"echo \ud800", "echo \u{fffd}"),
    (r"pair \ud83d\ude00", "pair \u{1f600}"),
    (r"high \ud800\u0041", "high \u{fffd}A"),
    (r"low \udc00", "low \u{fffd}"),
    (r"literal \\ud800", r"literal \ud800"),
  ];
  for (escaped, _) in cases {
    let payload = bash(escaped);
    let out = hook.run(Phase::Pre, &payload, &[&seen], &[]).result;
    let message: serde_json::Value = serde_json::from_str(&out.stdout).unwrap();
    assert_eq!(
      message["systemMessage"], payload,
      "modules get the raw payload"
    );
  }
  let expected: Vec<&str> = cases.iter().map(|(_, read)| *read).collect();
  assert_eq!(*seen.0.lock().unwrap(), expected);
}

#[test]
fn nesting_past_serdes_limit_fails_closed_before_and_after_a_tool() {
  let hook = Hook::new(Host::Claude).unwrap();
  let refused = |phase| hook.run(phase, &nested(126), &[], &[]).result;
  let seen = Seen(Mutex::new(Vec::new()));
  let deepest = hook.run(Phase::Pre, &nested(125), &[&seen], &[]).result;
  assert_eq!(
    (deepest.stdout.as_str(), deepest.exit_code),
    ("", 0),
    "125 arrays (127 levels) walk; serde refuses the 128th"
  );
  assert_eq!(
    *seen.0.lock().unwrap(),
    ["ls"],
    "the deepest payload reached the gates as Bash"
  );
  assert_eq!(
    refused(Phase::Pre).stdout,
    "{\"hookSpecificOutput\":{\"hookEventName\":\"PreToolUse\",\"permissionDecision\":\"deny\",\"permissionDecisionReason\":\"toolu: the hook payload nests too deeply to be checked\"}}\n"
  );
  assert_eq!(
    refused(Phase::Post).stdout,
    "{\"decision\":\"block\",\"reason\":\"toolu: the hook payload nests too deeply to be checked\"}\n"
  );
}

#[test]
fn a_disabled_hook_prints_nothing_whatever_the_payload() {
  let hook = Hook::new(Host::Claude).unwrap();
  let config = hook.sb.path("project/.claude/toolu.config.json");
  sandbox::write(&config, r#"{"version":1,"hooks":{"pre-tools":false}}"#).unwrap();
  let out = hook.run(Phase::Pre, &nested(200), &[], &[]).result;
  assert_eq!(
    (out.stdout.as_str(), out.stderr.as_str(), out.exit_code),
    ("", "", 0)
  );
}
