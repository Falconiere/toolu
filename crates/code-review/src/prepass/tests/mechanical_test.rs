use super::mechanical_checks_lines;
use crate::contract::{Severity, Source};
use crate::prepass::cap::cap;
use crate::prepass::tests::{empty, synthetic};
use crate::prepass::{Notice, NoticeKind, PrepassResult};

#[test]
fn prepass_mechanical_lists_findings_worst_first_then_notices() {
  let (kept, _) = cap(
    vec![synthetic(Source::Secrets, Severity::Medium, 1)],
    vec![synthetic(Source::AstGrep, Severity::Blocker, 2)],
    50,
  );
  let result = PrepassResult {
    findings: kept,
    redactions: Vec::new(),
    notices: vec![Notice {
      kind: NoticeKind::MissingAstGrep,
      text: "prepass: ast-grep not installed; structural rules skipped".to_owned(),
    }],
    overflow: false,
  };
  assert_eq!(
    mechanical_checks_lines(&result),
    [
      "[ast-grep/blocker] src/f002.rs:3 claim 2",
      "[secrets/medium] src/f001.rs:2 claim 1",
      "prepass: ast-grep not installed; structural rules skipped",
    ]
  );
}

#[test]
fn prepass_mechanical_empty_result_is_exactly_one_line() {
  assert_eq!(
    mechanical_checks_lines(&empty()),
    ["No mechanical findings."]
  );
}
