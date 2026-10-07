use super::{Folded, WalkState, event_name, stops_walk};
use crate::dispatch::output::parse_document;
use crate::dispatch::{ModuleResult, Phase};

fn module(stdout: &str, stderr: &str, exit_code: i32) -> Folded {
  Folded {
    name: "m".to_owned(),
    result: ModuleResult {
      stdout: stdout.to_owned(),
      stderr: stderr.to_owned(),
      exit_code,
    },
    truncated: false,
  }
}

const ASK: &str =
  r#"{"hookSpecificOutput":{"permissionDecision":"ask","permissionDecisionReason":"r"}}"#;
const DENY: &str =
  r#"{"hookSpecificOutput":{"permissionDecision":"deny","permissionDecisionReason":"no"}}"#;

#[test]
fn exit_2_ends_the_walk_with_every_stderr_so_far() {
  let mut state = WalkState::new(Phase::Pre);
  assert_eq!(state.consume(&module("", "lost\n", 3)), None);
  let done = state.consume(&module("", "kept\n", 2)).unwrap();
  assert_eq!(
    done.stderr,
    "toolu-dispatch: module m exited 3; output skipped\nkept\n"
  );
  assert_eq!(done.exit_code, 2);
}

#[test]
fn before_a_tool_the_first_ask_is_held_and_a_deny_ends_the_walk() {
  let mut state = WalkState::new(Phase::Pre);
  assert_eq!(state.consume(&module(ASK, "", 0)), None);
  assert_eq!(
    state.consume(&module(DENY, "", 0)).unwrap().stdout,
    format!("{DENY}\n")
  );
  let mut held = WalkState::new(Phase::Pre);
  held.consume(&module(ASK, "", 0));
  held.consume(&module(
    r#"{"hookSpecificOutput":{"permissionDecision":"ask","permissionDecisionReason":"second"}}"#,
    "",
    0,
  ));
  assert!(held.settle().stdout.contains("\"r\""));
}

#[test]
fn after_a_tool_only_a_block_ends_the_walk() {
  let mut state = WalkState::new(Phase::Post);
  assert_eq!(state.consume(&module(DENY, "", 0)), None);
  assert_eq!(state.consume(&module(ASK, "", 0)), None);
  let block = r#"{"decision":"block","reason":"bad"}"#;
  assert_eq!(
    state.consume(&module(block, "", 0)).unwrap().stdout,
    format!("{block}\n")
  );
  assert!(stops_walk(Phase::Post, parse_document(block).ok().as_ref()));
  assert!(!stops_walk(Phase::Pre, parse_document(block).ok().as_ref()));
}

#[test]
fn truncated_output_is_reported_and_settle_merges_advice() {
  let mut state = WalkState::new(Phase::Post);
  let mut cut = module(r#"{"systemMessage":"lost"}"#, "", 0);
  cut.truncated = true;
  assert_eq!(state.consume(&cut), None);
  state.consume(&module(r#"{"systemMessage":"kept"}"#, "", 0));
  let settled = state.settle();
  assert_eq!(
    settled.stderr,
    "toolu-dispatch: module m printed more than 8388608 bytes; output skipped\n"
  );
  assert_eq!(settled.stdout, "{\n  \"systemMessage\": \"kept\"\n}\n");
  assert_eq!(
    (event_name(Phase::Pre), event_name(Phase::Post)),
    ("PreToolUse", "PostToolUse")
  );
}
