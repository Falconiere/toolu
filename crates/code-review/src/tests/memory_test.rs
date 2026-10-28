use crate::contract::{Finding, Severity, Side, Source, Verdict};
use crate::validate::Recap;

use super::{BranchState, RecordInput, Status, dismiss, load, record};

fn finding() -> Finding {
  Finding {
    id: "f1".into(),
    path: "src/lib.rs".into(),
    line: 1,
    end_line: 1,
    side: Side::New,
    severity: Severity::High,
    category: "correctness".into(),
    source: Source::Llm,
    claim: "index underflows".into(),
    quote: "items[-1]".into(),
    suggestion: None,
    redaction_spans: Vec::new(),
  }
}

#[test]
fn passes_chain_by_diff_and_exact_dismissal_records_reason() {
  let dir = tempfile::tempdir().unwrap();
  let path = dir.path().join("code-review/feature.json");
  record(
    &path,
    &RecordInput {
      base: "main",
      sha: "sha-one",
      findings: &[finding()],
      recap: &Recap::default(),
      verdict: Verdict::Blocked,
      complete: true,
    },
  )
  .unwrap();
  let first: BranchState = load(&path).unwrap();
  assert_eq!(first.diff_sha, "sha-one");
  assert_eq!(first.passes.len(), 1);
  assert_eq!(first.memory.prior[0].status, Status::Open);
  assert!(dismiss(&path, "other", "not relevant").is_err());
  assert!(dismiss(&path, "prepass:secrets:src/lib.rs:1:secret", "safe").is_err());
  dismiss(&path, "f1", "guarded by caller").unwrap();
  let next = load(&path).unwrap();
  assert_eq!(next.memory.prior[0].status, Status::Dismissed);
  assert_eq!(next.memory.dismissals[0].reason, "guarded by caller");
  record(
    &path,
    &RecordInput {
      base: "main",
      sha: "sha-two",
      findings: &[],
      recap: &Recap::default(),
      verdict: Verdict::Approved,
      complete: true,
    },
  )
  .unwrap();
  let last = load(&path).unwrap();
  assert_eq!(
    last
      .passes
      .iter()
      .map(|pass| pass.diff_sha.as_str())
      .collect::<Vec<_>>(),
    ["sha-one", "sha-two"]
  );
}

#[test]
fn malformed_memory_fails_closed() {
  let dir = tempfile::tempdir().unwrap();
  let path = dir.path().join("bad.json");
  std::fs::write(&path, "{broken").unwrap();
  assert!(load(&path).is_err());
}

#[test]
fn a_late_record_does_not_reopen_a_dismissal() {
  let dir = tempfile::tempdir().unwrap();
  let path = dir.path().join("feature.json");
  let input = RecordInput {
    base: "main",
    sha: "sha",
    findings: &[finding()],
    recap: &Recap::default(),
    verdict: Verdict::Blocked,
    complete: true,
  };
  record(&path, &input).unwrap();
  dismiss(&path, "f1", "not applicable").unwrap();
  record(&path, &input).unwrap();
  let state = load(&path).unwrap();
  assert_eq!(state.memory.prior[0].status, Status::Dismissed);
  assert_eq!(state.memory.dismissals[0].reason, "not applicable");
}
