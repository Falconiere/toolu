use super::{error, findings, say};
use crate::Verdict;

#[test]
fn findings_decide_the_verdict() {
  assert_eq!(findings::<String>("task", &[]), Verdict::Clean);
  assert_eq!(findings("task", &["one".to_owned()]), Verdict::Findings);
}

#[test]
fn lines_can_always_be_written() {
  say("output test: stdout");
  error("output test: stderr");
}
