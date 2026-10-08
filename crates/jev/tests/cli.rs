//! The request and answer cases of `plugins/jev/hooks/src/__tests__/jev.test.ts`,
//! against the loopback endpoint (`failures.rs` has the failing ones).

#[path = "helpers/harness.rs"]
mod harness;

use harness::{ANSWER, Harness, PATH};
use serde_json::{Value, json};
use toolu_http_test_support::Reply;
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Outcome;

fn answer_for(id: &str) -> String {
  ANSWER.replace(r#""q":"#, &format!(r#""{id}":"#))
}

fn go(harness: &Harness, args: &[&str]) -> Outcome {
  harness.run(args, Some("fixture-key"), None, &[])
}

fn succeeded(outcome: &Outcome) {
  assert_eq!(outcome.exit, Exit::Success, "{outcome:?}");
  assert_eq!(outcome.stderr, None);
}

#[test]
fn noul_sends_one_typed_question_and_prints_compact_answers() {
  let harness = Harness::start(vec![Reply::new(200, ANSWER)]).expect("fixture");
  let outcome = go(
    &harness,
    &["noul", "-s", "Payouts failing for 3 days", "Urgent?"],
  );
  assert_eq!(
    outcome,
    Outcome {
      exit: Exit::Success,
      stdout: Some(r#"{"q":{"type":"noul","noul":0.92}}"#.to_owned()),
      stderr: None,
    }
  );
  let connects = harness.fixture.connects().expect("test setup");
  assert!(connects[0].starts_with("api.example.test:"), "{connects:?}");
  let requests = harness.fixture.requests().expect("test setup");
  assert_eq!(
    (requests[0].method.as_str(), requests[0].path.as_str()),
    ("POST", PATH)
  );
  assert_eq!(
    requests[0].headers.get("authorization").map(String::as_str),
    Some("Bearer fixture-key")
  );
  assert_eq!(
    harness.bodies(),
    [
      r#"{"state":"Payouts failing for 3 days","model":"jev-latest","questions":{"q":{"type":"noul","instructions":"Urgent?"}}}"#
    ]
  );
}

#[test]
fn noul_sides_and_the_id_shape_the_question() {
  let harness = Harness::start(vec![Reply::new(200, answer_for("urgent"))]).expect("fixture");
  let args = [
    "noul", "-s", "x", "Urgent?", "--true", "yes", "--false", "no", "--id", "urgent",
  ];
  succeeded(&go(&harness, &args));
  assert_eq!(
    harness.last()["questions"],
    json!({ "urgent": { "type": "noul", "instructions": "Urgent?",
                        "criteria": { "true": "yes", "false": "no" } } })
  );
}

#[test]
fn choice_keeps_its_criteria_shape_and_prints_its_answers() {
  let reply = json!({ "model": "jev-1.13.0", "usage": { "input_tokens": 3, "output_tokens": 2 },
    "answers": { "q": { "type": "choice", "choice": "billing",
      "probabilities": { "billing": 0.8, "tech": 0.2 }, "confidence": 0.8 } } });
  let harness = Harness::start(vec![Reply::new(200, reply.to_string())]).expect("fixture");
  let args = [
    "choice",
    "-s",
    "ticket",
    "Which team?",
    "-o",
    "billing=Payments",
    "-o",
    "tech",
  ];
  let outcome = go(&harness, &args);
  succeeded(&outcome);
  assert_eq!(
    harness.last()["questions"]["q"]["criteria"],
    json!({ "billing": "Payments", "tech": null })
  );
  let printed: Value =
    serde_json::from_str(&outcome.stdout.expect("test setup")).expect("test setup");
  assert_eq!(printed, reply["answers"]);
}

#[test]
fn score_keeps_its_levels_in_order() {
  let reply = json!({ "model": "jev-1.13.0", "usage": { "input_tokens": 3, "output_tokens": 2 },
    "answers": { "q": { "type": "score", "score": 1.5,
      "legend": { "0": "Calm", "1": "Angry", "2": "Furious" },
      "probabilities": { "0": 0.1, "1": 0.3, "2": 0.6 }, "confidence": 0.8 } } });
  let harness = Harness::start(vec![Reply::new(200, reply.to_string())]).expect("fixture");
  let args = [
    "score",
    "-s",
    "ticket",
    "Severity?",
    "-l",
    "Calm",
    "-l",
    "Angry",
    "-l",
    "Furious",
  ];
  succeeded(&go(&harness, &args));
  assert_eq!(
    harness.last()["questions"]["q"]["criteria"],
    json!(["Calm", "Angry", "Furious"])
  );
}

#[test]
fn ask_reads_a_questions_file_and_raw_prints_the_whole_body() {
  let dir = tempfile::tempdir().expect("test setup");
  let file = dir.path().join("questions.json");
  std::fs::write(
    &file,
    r#"{"urgent":{"type":"noul","instructions":"Urgent?"}}"#,
  )
  .expect("test setup");
  let harness = Harness::start(vec![Reply::new(200, answer_for("urgent"))]).expect("fixture");
  let outcome = go(
    &harness,
    &[
      "ask",
      file.to_str().expect("test setup"),
      "-s",
      "ticket",
      "--raw",
    ],
  );
  succeeded(&outcome);
  let printed = outcome.stdout.expect("test setup");
  assert_eq!(
    serde_json::from_str::<Value>(&printed).expect("test setup"),
    serde_json::from_str::<Value>(&answer_for("urgent")).expect("test setup")
  );
  assert!(
    printed.starts_with("{\n  \"model\": \"jev-1.13.0\","),
    "{printed}"
  );
  assert!(!printed.ends_with('\n'));
  assert_eq!(
    harness.last()["questions"],
    json!({ "urgent": { "type": "noul", "instructions": "Urgent?" } })
  );
}

#[test]
fn a_choice_option_named_proto_remains_a_data_key() {
  let reply = json!({ "model": "jev-1.13.0", "usage": { "input_tokens": 3, "output_tokens": 2 },
    "answers": { "q": { "type": "choice", "choice": "__proto__",
      "probabilities": { "__proto__": 0.6, "other": 0.4 }, "confidence": 0.6 } } });
  let harness = Harness::start(vec![Reply::new(200, reply.to_string())]).expect("fixture");
  let args = [
    "choice",
    "-s",
    "ticket",
    "Which?",
    "-o",
    "__proto__=prototype",
    "-o",
    "other",
  ];
  succeeded(&go(&harness, &args));
  let body = harness.bodies().remove(0);
  assert!(
    body.contains(r#""criteria":{"__proto__":"prototype","other":null}"#),
    "{body}"
  );
}

#[test]
fn ask_reads_a_question_map_from_stdin_and_pins_the_model() {
  let harness = Harness::start(vec![Reply::new(200, answer_for("urgent"))]).expect("fixture");
  let stdin = r#"{"urgent":{"type":"noul","instructions":"Urgent?"}}"#;
  let args = ["ask", "-", "-s", "ticket", "-m", "jev-1.13"];
  let outcome = harness.run(&args, Some("fixture-key"), Some(stdin), &[]);
  succeeded(&outcome);
  assert_eq!(
    harness.last(),
    json!({ "state": "ticket", "model": "jev-1.13",
            "questions": { "urgent": { "type": "noul", "instructions": "Urgent?" } } })
  );
}

#[test]
fn structured_file_state_stays_structured_and_scalar_file_and_stdin_stay_strings() {
  let dir = tempfile::tempdir().expect("test setup");
  let file = dir.path().join("state.json");
  let at = format!("@{}", file.display());
  let harness = Harness::start(vec![Reply::new(200, ANSWER)]).expect("fixture");
  std::fs::write(&file, "{\"ticket\":\"Payouts failing\"}\n").expect("test setup");
  succeeded(&go(&harness, &["noul", "-s", &at, "Urgent?"]));
  assert_eq!(
    harness.last()["state"],
    json!({ "ticket": "Payouts failing" })
  );
  std::fs::write(&file, "123\n").expect("test setup");
  succeeded(&go(&harness, &["noul", "-s", &at, "Urgent?"]));
  assert_eq!(harness.last()["state"], json!("123"));
  let outcome = harness.run(
    &["noul", "-s", "-", "Urgent?"],
    Some("fixture-key"),
    Some("Payouts failing\n"),
    &[],
  );
  succeeded(&outcome);
  assert_eq!(harness.last()["state"], json!("Payouts failing"));
}
