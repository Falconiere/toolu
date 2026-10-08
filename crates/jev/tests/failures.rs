//! The failing cases of `plugins/jev/hooks/src/__tests__/jev.test.ts`: what the
//! old curl statuses 1, 22 and 28 became, and that no failure prints a judgment.

#[path = "helpers/harness.rs"]
mod harness;

use clap::error::ErrorKind;
use harness::{ANSWER, Harness};
use serde_json::{Value, json};
use toolu_http_test_support::Reply;
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Outcome;

const NOUL: [&str; 4] = ["noul", "-s", "x", "Urgent?"];

fn answer_with(patch: &Value) -> String {
  let mut body: Value = serde_json::from_str(ANSWER).unwrap_or(Value::Null);
  if let (Some(map), Some(patch)) = (body.as_object_mut(), patch.as_object()) {
    for (key, value) in patch {
      map.insert(key.clone(), value.clone());
    }
  }
  body.to_string()
}

fn run(harness: &Harness, args: &[&str]) -> Outcome {
  harness.run(args, Some("fixture-key"), None, &[])
}

fn failed(outcome: &Outcome, exit: Exit, stderr: &str) {
  assert_eq!(outcome.exit, exit, "{outcome:?}");
  assert_eq!(outcome.stdout, None);
  assert_eq!(outcome.stderr.as_deref(), Some(stderr));
}

fn invalid(outcome: &Outcome) {
  assert_eq!(outcome.exit, Exit::Failure, "{outcome:?}");
  assert_eq!(outcome.stdout, None);
  assert!(
    outcome
      .stderr
      .as_deref()
      .unwrap_or_default()
      .contains("invalid response"),
    "{outcome:?}"
  );
}

#[test]
fn a_missing_or_empty_key_fails_before_any_request() {
  let harness = Harness::start(vec![Reply::new(200, ANSWER)]).expect("fixture");
  for key in [None, Some("")] {
    let outcome = harness.run(&NOUL, key, None, &[]);
    failed(&outcome, Exit::Failure, "jev: TYPESAFE_API_KEY unset");
  }
  assert_eq!(harness.bodies().len(), 0);
}

#[test]
fn a_key_with_a_line_break_fails_before_any_request() {
  let harness = Harness::start(vec![Reply::new(200, ANSWER)]).expect("fixture");
  for key in ["bad\nkey", "bad\rkey"] {
    let outcome = harness.run(&NOUL, Some(key), None, &[]);
    failed(
      &outcome,
      Exit::Failure,
      "jev: TYPESAFE_API_KEY must not contain line breaks",
    );
  }
  assert_eq!(harness.bodies().len(), 0);
}

#[test]
fn invalid_arguments_send_no_request() {
  let harness = Harness::start(vec![Reply::new(200, ANSWER)]).expect("fixture");
  // Clap's usage errors, which the binary exits 64 on.
  for (args, kind) in [
    (vec!["noul", "Urgent?"], ErrorKind::MissingRequiredArgument),
    (
      vec!["noul", "-s", "x", "Urgent?", "--weight", "3"],
      ErrorKind::UnknownArgument,
    ),
    (vec!["ask", "--raw"], ErrorKind::MissingRequiredArgument),
    (vec![], ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand),
  ] {
    let err = harness
      .try_run(&args, Some("fixture-key"), None, &[])
      .unwrap_err();
    assert_eq!(err.kind(), kind, "{args:?}");
    let outcome = harness.run(&args, Some("fixture-key"), None, &[]);
    assert_eq!(
      (outcome.exit, outcome.stdout),
      (Exit::Usage, None),
      "{args:?}"
    );
  }
  // The client's counts and one stdin reader, which exit 1.
  for args in [
    vec!["choice", "-s", "x", "Which?", "-o", "one"],
    vec!["score", "-s", "x", "How?", "-l", "low"],
  ] {
    let outcome = harness.run(&args, Some("fixture-key"), None, &[]);
    assert_eq!(outcome.exit, Exit::Failure, "{args:?}");
    assert_eq!(outcome.stdout, None);
  }
  let outcome = harness.run(
    &["ask", "-", "-s", "-"],
    Some("fixture-key"),
    Some("{}"),
    &[],
  );
  failed(
    &outcome,
    Exit::Failure,
    "jev: only one option can read stdin",
  );
  assert_eq!(harness.bodies().len(), 0);
}

#[test]
fn unreadable_inputs_fail_with_their_name() {
  let harness = Harness::start(vec![Reply::new(200, ANSWER)]).expect("fixture");
  let outcome = run(&harness, &["noul", "-s", "@/nonexistent/state", "Urgent?"]);
  failed(
    &outcome,
    Exit::Failure,
    "jev: cannot read /nonexistent/state",
  );
  let outcome = run(&harness, &["ask", "/nonexistent/q.json", "-s", "x"]);
  failed(
    &outcome,
    Exit::Failure,
    "jev: cannot read /nonexistent/q.json",
  );
  failed(
    &run(&harness, &["ask", "-", "-s", "x"]),
    Exit::Failure,
    "jev: cannot read stdin",
  );
  assert_eq!(harness.bodies().len(), 0);
}

#[test]
fn choice_and_score_enforce_upper_bounds_before_network() {
  let harness = Harness::start(vec![Reply::new(200, ANSWER)]).expect("fixture");
  let keys: Vec<String> = (0..256).map(|i| format!("key{i}")).collect();
  let mut choice = vec!["choice", "-s", "x", "Which?"];
  for key in &keys {
    choice.extend(["-o", key]);
  }
  failed(
    &run(&harness, &choice),
    Exit::Failure,
    "jev: choice accepts at most 255 options",
  );
  let levels: Vec<String> = (0..11).map(|i| i.to_string()).collect();
  let mut score = vec!["score", "-s", "x", "How?"];
  for level in &levels {
    score.extend(["-l", level]);
  }
  failed(
    &run(&harness, &score),
    Exit::Failure,
    "jev: score accepts at most 10 levels",
  );
  assert_eq!(harness.bodies().len(), 0);
}

#[test]
fn a_malformed_success_cannot_become_a_judgment() {
  let wrong_noul = answer_with(&json!({ "answers": { "q": { "type": "noul", "noul": 1.2 } } }));
  for body in ["not json", r#"{"model":"x"}"#, &wrong_noul] {
    let harness = Harness::start(vec![Reply::new(200, body)]).expect("fixture");
    invalid(&run(&harness, &NOUL));
  }
}

#[test]
fn a_missing_answer_wrong_type_and_invalid_distribution_fail_closed() {
  let choice = answer_with(
    &json!({ "answers": { "q": { "type": "choice", "choice": "a",
    "probabilities": { "a": 0.8, "b": 0.1 }, "confidence": 0.8 } } }),
  );
  let choice_args = ["choice", "-s", "x", "Which?", "-o", "a", "-o", "b"];
  let cases = [
    (answer_with(&json!({ "answers": {} })), NOUL.to_vec()),
    (
      answer_with(&json!({ "answers": { "q": { "type": "choice", "choice": "a" } } })),
      NOUL.to_vec(),
    ),
    (choice, choice_args.to_vec()),
    (
      answer_with(&json!({ "usage": { "input_tokens": -1, "output_tokens": 2 } })),
      NOUL.to_vec(),
    ),
  ];
  for (body, args) in cases {
    let harness = Harness::start(vec![Reply::new(200, body)]).expect("fixture");
    invalid(&run(&harness, &args));
  }
}

#[test]
fn a_terminal_http_error_prints_its_body_on_stderr_and_exits_1() {
  let harness = Harness::start(vec![Reply::new(401, r#"{"error":"bad key"}"#)]).expect("fixture");
  failed(
    &run(&harness, &NOUL),
    Exit::Failure,
    r#"{"error":"bad key"}"#,
  );
  assert_eq!(harness.bodies().len(), 1);
}

#[test]
fn a_408_and_a_529_retry_then_a_typed_answer_succeeds() {
  let harness = Harness::start(vec![
    Reply::new(408, "transient"),
    Reply::new(529, "overloaded"),
    Reply::new(200, ANSWER),
  ])
  .expect("fixture");
  let outcome = run(&harness, &NOUL);
  assert_eq!(outcome.exit, Exit::Success, "{outcome:?}");
  assert_eq!(outcome.stderr, None);
  assert_eq!(harness.bodies().len(), 3);
  assert_eq!(harness.last()["questions"]["q"]["instructions"], "Urgent?");
}

#[test]
fn a_retry_after_over_60_seconds_surfaces_the_body_without_waiting() {
  let harness =
    Harness::start(vec![Reply::new(429, "busy").header("Retry-After", "61")]).expect("fixture");
  failed(&run(&harness, &NOUL), Exit::Failure, "busy");
  assert_eq!(harness.bodies().len(), 1);
}

#[test]
fn a_dropped_connection_is_retried_to_a_valid_answer() {
  let harness = Harness::start(vec![Reply::dropped(), Reply::new(200, ANSWER)]).expect("fixture");
  assert_eq!(run(&harness, &NOUL).exit, Exit::Success);
  assert!(harness.fixture.connects().expect("test setup").len() >= 2);
  assert_eq!(harness.bodies().len(), 1);
}

#[test]
fn a_zero_second_timeout_exits_temp_fail_after_the_retries() {
  let harness = Harness::start(vec![Reply::new(200, ANSWER)]).expect("fixture");
  let outcome = harness.run(&NOUL, Some("fixture-key"), None, &[("JEV_TIMEOUT", "0")]);
  failed(&outcome, Exit::TempFail, "jev: the Jev request timed out");
  assert_eq!(harness.bodies().len(), 0);
}

#[test]
fn the_key_never_appears_in_a_failure() {
  let harness = Harness::start(vec![Reply::new(401, "rejected sk-secret-key")]).expect("fixture");
  let outcome = harness.run(&NOUL, Some("sk-secret-key"), None, &[]);
  assert_eq!(outcome.exit, Exit::Failure);
  assert_eq!(outcome.stdout, None);
  assert!(
    !outcome
      .stderr
      .expect("test setup")
      .contains("sk-secret-key")
  );
}
