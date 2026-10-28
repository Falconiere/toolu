use super::prepass_degraded;
use crate::prepass::{Notice, NoticeKind, PrepassResult};

#[test]
fn failed_scanner_blocks_complete_review_but_overflow_notice_alone_does_not() {
  let mut result = PrepassResult {
    findings: Vec::new(),
    redactions: Vec::new(),
    notices: Vec::new(),
    overflow: false,
  };
  assert!(!prepass_degraded(&result));
  result.notices.push(Notice {
    kind: NoticeKind::AstGrepFailed,
    text: "scanner failed".into(),
  });
  assert!(prepass_degraded(&result));
  result.notices.clear();
  result.notices.push(Notice {
    kind: NoticeKind::Overflow,
    text: "capped".into(),
  });
  assert!(!prepass_degraded(&result));
  result.overflow = true;
  assert!(prepass_degraded(&result));
}

#[test]
fn settle_blocks_an_incomplete_review_and_leaves_a_complete_one_alone() {
  use super::settle;
  use crate::budget::Effort;
  use crate::contract::{Coverage, Omitted, Report, Usage, Verdict};

  let mut partial = Report::reviewed(
    Effort::Medium,
    Vec::new(),
    Coverage {
      reviewed: Vec::new(),
      unreviewed: vec![Omitted {
        path: "src/lib.rs".to_owned(),
        reason: "lost".to_owned(),
      }],
      skipped: Vec::new(),
    },
    Usage::default(),
  );
  assert!(!settle(&mut partial));
  assert_eq!(partial.blocking_count(), 1);
  assert_eq!(partial.findings[0].id, "code-review:incomplete");
  let mut whole = Report::reviewed(
    Effort::Medium,
    Vec::new(),
    Coverage::default(),
    Usage::default(),
  );
  assert!(settle(&mut whole));
  assert_eq!(whole.verdict(), Verdict::Approved);
  assert_eq!(whole.findings, Vec::new());
}
