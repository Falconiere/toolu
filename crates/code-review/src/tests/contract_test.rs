use toolu_protocol::exit::Exit;

use super::{Cost, Coverage, ErrorKind, Finding, Omitted, Report, Severity, Side, Source, Usage};
use crate::budget::Effort;

fn finding(severity: Severity) -> Finding {
  Finding {
    id: "f1".to_owned(),
    path: "src/lib.rs".to_owned(),
    line: 3,
    end_line: 3,
    side: Side::New,
    severity,
    category: "correctness".to_owned(),
    source: Source::Llm,
    claim: "claim".to_owned(),
    quote: "quote".to_owned(),
    suggestion: None,
  }
}

fn reviewed(findings: Vec<Finding>, coverage: Coverage) -> Report {
  Report::reviewed(Effort::Medium, findings, coverage, Usage::default())
}

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
fn the_verdict_is_computed_from_findings_and_coverage() {
  let advisory = reviewed(
    vec![finding(Severity::Low), finding(Severity::Nit)],
    Coverage::default(),
  );
  assert_eq!(
    (advisory.verdict.name(), advisory.exit()),
    ("approved", Exit::Success)
  );
  assert_eq!(advisory.blocking_count(), 0);
  let blocked = reviewed(
    vec![finding(Severity::Medium), finding(Severity::Low)],
    Coverage::default(),
  );
  assert_eq!(
    (blocked.verdict.name(), blocked.exit()),
    ("blocked", Exit::Blocked)
  );
  assert_eq!(blocked.blocking_count(), 1);
  let sorted = reviewed(
    vec![finding(Severity::Nit), finding(Severity::Blocker)],
    Coverage::default(),
  );
  assert_eq!(
    sorted
      .findings
      .iter()
      .map(|f| f.severity)
      .collect::<Vec<_>>(),
    [Severity::Blocker, Severity::Nit]
  );
  let partial = reviewed(
    Vec::new(),
    Coverage {
      unreviewed: vec![Omitted {
        path: "src/big.rs".to_owned(),
        reason: "over budget".to_owned(),
      }],
      ..Coverage::default()
    },
  );
  assert_eq!((partial.verdict.name(), partial.error), ("blocked", None));
  assert_eq!(Exit::Blocked.code(), 2);
}

#[test]
fn a_failed_report_carries_the_versions_and_its_error() {
  let report = Report::failed(Effort::High, ErrorKind::Outage, "every call failed");
  assert_eq!(
    (report.verdict.name(), report.exit()),
    ("error", Exit::Unavailable)
  );
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

#[test]
fn a_non_finite_or_negative_cost_does_not_serialise() {
  for cost_usd in [f64::NAN, f64::INFINITY, -0.5] {
    let usage = Usage {
      cost_usd: Cost(cost_usd),
      ..Usage::default()
    };
    let report = Report::reviewed(Effort::Small, Vec::new(), Coverage::default(), usage);
    let err = serde_json::to_string(&report).unwrap_err();
    assert!(err.to_string().contains("cost_usd"), "{err}");
  }
  let report = reviewed(Vec::new(), Coverage::default());
  assert!(
    serde_json::to_string(&report)
      .unwrap()
      .contains("\"cost_usd\":0.0")
  );
}
