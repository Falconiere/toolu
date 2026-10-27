use toolu_protocol::exit::Exit;

use super::{ErrorKind, Report, Severity, Verdict};
use crate::budget::Effort;

#[test]
fn medium_and_above_block_and_low_and_nit_advise() {
  let blocking: Vec<bool> = [
    Severity::Blocker,
    Severity::High,
    Severity::Medium,
    Severity::Low,
    Severity::Nit,
  ]
  .map(Severity::blocking)
  .into();
  assert_eq!(blocking, [true, true, true, false, false]);
}

#[test]
fn the_verdict_maps_to_the_documented_exit() {
  let mut report = Report::failed(Effort::Auto, ErrorKind::Unconfigured, "why");
  assert_eq!(
    (report.exit(), report.verdict.name()),
    (Exit::Unavailable, "error")
  );
  report.verdict = Verdict::Blocked;
  assert_eq!(
    (report.exit(), report.verdict.name()),
    (Exit::Blocked, "blocked")
  );
  report.verdict = Verdict::Approved;
  assert_eq!(
    (report.exit(), report.verdict.name()),
    (Exit::Success, "approved")
  );
  assert_eq!(Exit::Blocked.code(), 2);
}

#[test]
fn a_failed_report_carries_the_versions_and_its_error() {
  let report = Report::failed(Effort::High, ErrorKind::Outage, "every call failed");
  assert_eq!(report.schema_version, super::SCHEMA_VERSION);
  assert_eq!(report.toolu_version, env!("CARGO_PKG_VERSION"));
  assert_eq!(report.blocking_count(), 0);
  let error = report.error.unwrap();
  assert_eq!(
    (error.kind.name(), error.message.as_str()),
    ("outage", "every call failed")
  );
  assert_eq!(ErrorKind::Unconfigured.name(), "unconfigured");
}
