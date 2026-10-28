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
