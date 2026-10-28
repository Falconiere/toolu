use super::cap;
use crate::budget::PREPASS_FINDINGS_MAX;
use crate::contract::{Severity, Source};
use crate::prepass::NoticeKind;
use crate::prepass::tests::{batch, synthetic};

#[test]
fn prepass_cap_prefers_secrets_and_reports_the_overflow() {
  let secrets = batch(Source::Secrets, Severity::Blocker, 40);
  let structural = batch(Source::AstGrep, Severity::Medium, 30);
  let (kept, notice) = cap(secrets, structural, PREPASS_FINDINGS_MAX);
  assert_eq!(kept.len(), PREPASS_FINDINGS_MAX);
  assert_eq!(
    kept.iter().filter(|f| f.source == Source::Secrets).count(),
    40
  );
  let notice = notice.expect("overflow notice");
  assert_eq!(notice.kind, NoticeKind::Overflow);
  assert_eq!(notice.text, "prepass: capped at 50 findings (dropped 20)");
}

#[test]
fn prepass_cap_drops_ast_grep_entirely_when_secrets_fill_it() {
  let secrets = batch(Source::Secrets, Severity::Medium, 60);
  let structural = batch(Source::AstGrep, Severity::Medium, 5);
  let (kept, notice) = cap(secrets, structural, 50);
  assert!(kept.iter().all(|finding| finding.source == Source::Secrets));
  assert_eq!(kept.len(), 50);
  assert_eq!(
    notice.map(|notice| notice.text),
    Some("prepass: capped at 50 findings (dropped 15)".to_owned())
  );
}

#[test]
fn prepass_cap_keeps_the_worst_first_and_stays_silent_under_the_cap() {
  let mut secrets = batch(Source::Secrets, Severity::Medium, 2);
  secrets.push(synthetic(Source::Secrets, Severity::Blocker, 9));
  let (kept, notice) = cap(secrets, Vec::new(), 50);
  assert_eq!(kept[0].severity, Severity::Blocker);
  assert_eq!(notice, None);
  let (kept, notice) = cap(batch(Source::Secrets, Severity::Low, 1), Vec::new(), 1);
  assert_eq!((kept.len(), notice), (1, None));
}
