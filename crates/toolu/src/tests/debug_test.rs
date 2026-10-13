use toolu_runtime::cli::Ctx;

use super::{command, io, records, run};

#[test]
fn debug_lists_four_real_verbs() {
  for verb in ["io", "log", "stack", "testfail"] {
    assert!(command().try_get_matches_from(["debug", verb]).is_ok());
  }
  assert!(
    command()
      .try_get_matches_from(["debug", "planned"])
      .is_err()
  );
}

#[test]
fn empty_and_final_newline_have_the_expected_record_count() {
  assert_eq!(records(""), Vec::<&str>::new());
  assert_eq!(records("one\ntwo\n"), ["one", "two"]);
  assert_eq!(
    serde_json::from_str::<serde_json::Value>(&io(b"one\ntwo\n", true)).unwrap(),
    serde_json::json!({"bytes":8,"lines":2})
  );
}

#[test]
fn io_reads_the_context_stdin_and_missing_stdin_fails() {
  let matches = command().try_get_matches_from(["debug", "io"]).unwrap();
  let supplied = Ctx {
    stdin: Some("a\nb\n".to_owned()),
    json: true,
    ..Ctx::default()
  };
  let answer = run(&matches, &supplied);
  assert_eq!(answer.stdout.as_deref(), Some(r#"{"bytes":4,"lines":2}"#));
  assert_ne!(
    run(&matches, &Ctx::default()).exit,
    toolu_protocol::exit::Exit::Success
  );
}
