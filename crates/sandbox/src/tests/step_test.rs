use serde_json::json;
use toolu_protocol::exit::Exit;
use toolu_runtime::process::RunError;

use super::{Failure, Report, describe};

#[test]
fn a_report_prints_its_document_under_json_and_its_text_otherwise() {
  let report = Report {
    exit: Exit::Unavailable,
    doc: json!({ "namespace": "sandbox" }),
    text: "sandbox: text".to_owned(),
  };
  let json = report.clone().outcome(true);
  assert_eq!(json.exit, Exit::Unavailable);
  assert_eq!(json.stdout.as_deref(), Some(r#"{"namespace":"sandbox"}"#));
  assert_eq!(json.stderr, None);
  assert_eq!(
    report.outcome(false).stdout.as_deref(),
    Some("sandbox: text")
  );
}

#[test]
fn a_failure_goes_to_stderr_with_the_namespace() {
  let outcome = Failure::new(Exit::Usage, "nope").outcome();
  assert_eq!(outcome.exit, Exit::Usage);
  assert_eq!(outcome.stdout, None);
  assert_eq!(outcome.stderr.as_deref(), Some("sandbox: nope"));
}

#[test]
fn every_process_error_reads_as_a_phrase() {
  assert_eq!(describe(&RunError::EmptyArgv), "no command was given");
  assert_eq!(describe(&RunError::ZeroTimeout), "the deadline is unusable");
  assert_eq!(
    describe(&RunError::TimeoutTooLong),
    "the deadline is unusable"
  );
  for err in [
    RunError::Spawn("x: gone".to_owned()),
    RunError::Wait("x: gone".to_owned()),
    RunError::Stdin("x: gone".to_owned()),
    RunError::Callback("x: gone".to_owned()),
  ] {
    assert_eq!(describe(&err), "x: gone");
  }
}
