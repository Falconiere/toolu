use std::fs;

use clap::ArgMatches;

use super::{CallError, build, needs_stdin};
use crate::cli::command;

fn matches(args: &[&str]) -> ArgMatches {
  let mut words = vec!["jev"];
  words.extend_from_slice(args);
  command().try_get_matches_from(words).unwrap()
}

fn questions(args: &[&str], stdin: Option<&str>) -> Result<String, CallError> {
  build(&matches(args), stdin).map(|call| format!("{:?}", call.questions))
}

#[test]
fn noul_builds_one_question_under_its_id() {
  let call = build(
    &matches(&["noul", "-s", "x", "--id", "z", "--true", "yes", "Urgent?"]),
    None,
  )
  .unwrap();
  assert_eq!(call.model, "jev-latest");
  assert!(!call.raw);
  let shown = format!("{:?}", call.questions);
  assert!(shown.contains("\"z\"") && shown.contains("yes"), "{shown}");
}

#[test]
fn choice_options_split_at_the_first_equals_and_score_levels_keep_order() {
  let shown = questions(
    &["choice", "-s", "x", "Which?", "-o", "a=b=c", "-o", "d"],
    None,
  )
  .unwrap();
  assert!(shown.contains('a') && shown.contains("b=c"), "{shown}");
  let shown = questions(
    &["score", "-s", "x", "How?", "-l", "Calm", "-l", "Angry"],
    None,
  )
  .unwrap();
  assert!(shown.find("Calm") < shown.find("Angry"), "{shown}");
}

#[test]
fn the_client_bounds_surface_as_client_errors() {
  let one = questions(&["choice", "-s", "x", "Which?", "-o", "a"], None);
  assert!(matches!(one, Err(CallError::Client(_))));
  let none = questions(&["noul", "-s", "x", ""], None);
  assert!(matches!(none, Err(CallError::Client(_))));
}

#[test]
fn state_reads_stdin_or_a_file_and_stays_text_otherwise() {
  let dir = tempfile::tempdir().unwrap();
  let file = dir.path().join("state.json");
  fs::write(&file, "{\"a\":1}\n").unwrap();
  let from_file = format!("@{}", file.display());
  let state = |arg: &str, stdin: Option<&str>| {
    build(&matches(&["noul", "-s", arg, "Urgent?"]), stdin).map(|call| format!("{:?}", call.state))
  };
  assert!(state(&from_file, None).unwrap().contains("Object"));
  assert!(state("-", Some("{\"a\":1}\n")).unwrap().contains("Object"));
  assert!(
    state("-", Some("123\n"))
      .unwrap()
      .contains("String(\"123\")")
  );
  assert!(
    state("plain text", None)
      .unwrap()
      .contains("String(\"plain text\")")
  );
  assert!(
    state("-s text", None)
      .unwrap()
      .contains("String(\"-s text\")")
  );
  assert_eq!(
    state("-", None),
    Err(CallError::Input("cannot read stdin".into()))
  );
  let missing = format!("@{}/nope", dir.path().display());
  assert_eq!(
    state(&missing, None),
    Err(CallError::Input(format!(
      "cannot read {}/nope",
      dir.path().display()
    )))
  );
}

#[test]
fn ask_reads_a_file_or_stdin_and_refuses_two_stdin_readers() {
  let dir = tempfile::tempdir().unwrap();
  let file = dir.path().join("q.json");
  fs::write(
    &file,
    "{\"u\":{\"type\":\"noul\",\"instructions\":\"Urgent?\"}}\n",
  )
  .unwrap();
  let path = file.to_str().unwrap();
  assert!(
    questions(&["ask", path, "-s", "x"], None)
      .unwrap()
      .contains("Urgent?")
  );
  let piped = r#"{"v":{"type":"noul","instructions":"Now?"}}"#;
  assert!(
    questions(&["ask", "-", "-s", "x"], Some(piped))
      .unwrap()
      .contains("Now?")
  );
  assert_eq!(
    questions(&["ask", "-", "-s", "-"], Some(piped)),
    Err(CallError::Input("only one option can read stdin".into()))
  );
  assert_eq!(
    questions(&["ask", "-", "-s", "x"], None),
    Err(CallError::Input("cannot read stdin".into()))
  );
  assert!(matches!(
    questions(&["ask", "-", "-s", "x"], Some("[]")),
    Err(CallError::Client(_))
  ));
}

#[test]
fn needs_stdin_ignores_a_missing_verb() {
  assert!(!needs_stdin(&ArgMatches::default()));
}
