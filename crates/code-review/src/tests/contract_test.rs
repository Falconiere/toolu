use toolu_protocol::exit::Exit;

use super::{
  Cost, Coverage, ErrorKind, Finding, Omitted, RedactionSpan, Report, Severity, Side, Source, Usage,
};
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
    redaction_spans: Vec::new(),
    assessment: None,
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
fn severities_order_from_nit_up() {
  assert!(Severity::Nit < Severity::Low && Severity::Low < Severity::Medium);
  assert!(Severity::Medium < Severity::High && Severity::High < Severity::Blocker);
}

#[test]
fn the_verdict_is_computed_from_findings_and_coverage() {
  let advisory = reviewed(
    vec![finding(Severity::Low), finding(Severity::Nit)],
    Coverage::default(),
  );
  assert_eq!(
    (advisory.verdict().name(), advisory.exit()),
    ("approved", Exit::Success)
  );
  assert_eq!(advisory.blocking_count(), 0);
  let mut blocked = reviewed(
    vec![finding(Severity::Medium), finding(Severity::Low)],
    Coverage::default(),
  );
  assert_eq!(
    (blocked.verdict().name(), blocked.exit()),
    ("blocked", Exit::Blocked)
  );
  assert_eq!(blocked.blocking_count(), 1);
  blocked
    .findings
    .retain(|finding| !finding.severity.blocking());
  assert_eq!(
    (blocked.verdict().name(), blocked.exit()),
    ("approved", Exit::Success)
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
  assert_eq!((partial.verdict().name(), partial.error), ("blocked", None));
  assert_eq!(Exit::Blocked.code(), 2);
}

#[test]
fn the_document_prints_findings_worst_first_with_its_versions() {
  let report = reviewed(
    vec![
      finding(Severity::Nit),
      finding(Severity::Blocker),
      finding(Severity::Low),
      finding(Severity::High),
    ],
    Coverage::default(),
  );
  let doc = serde_json::to_value(&report).unwrap();
  let order: Vec<&str> = doc["findings"]
    .as_array()
    .unwrap()
    .iter()
    .map(|finding| finding["severity"].as_str().unwrap())
    .collect();
  assert_eq!(order, ["blocker", "high", "low", "nit"]);
  assert_eq!(doc["schema_version"], super::SCHEMA_VERSION);
  assert_eq!(doc["schema_version"], 3);
  assert_eq!(doc["toolu_version"], env!("CARGO_PKG_VERSION"));
  assert_eq!(
    (doc["verdict"].as_str(), &doc["error"]),
    (Some("blocked"), &serde_json::Value::Null)
  );
}

#[test]
fn a_failed_report_carries_its_error() {
  let report = Report::failed(Effort::High, ErrorKind::Outage, "every call failed");
  assert_eq!(
    (report.verdict().name(), report.exit()),
    ("error", Exit::Unavailable)
  );
  assert_eq!(report.blocking_count(), 0);
  let error = report.error.unwrap();
  assert_eq!(
    (error.kind.name(), error.message.as_str()),
    ("outage", "every call failed")
  );
  let kinds = [
    ErrorKind::Unconfigured,
    ErrorKind::NotEnabled,
    ErrorKind::Outage,
    ErrorKind::Deadline,
    ErrorKind::State,
  ];
  for kind in kinds {
    assert_eq!(
      serde_json::to_value(kind).unwrap(),
      serde_json::json!(kind.name())
    );
  }
  assert_eq!(ErrorKind::NotEnabled.name(), "not-enabled");
}

#[test]
fn a_cost_is_finite_and_non_negative() {
  for usd in [f64::NAN, f64::INFINITY, -0.5] {
    assert_eq!(Cost::usd(usd), None, "{usd}");
  }
  let usage = Usage {
    cost_usd: Cost::usd(0.25),
    ..Usage::default()
  };
  let report = Report::reviewed(Effort::Small, Vec::new(), Coverage::default(), usage);
  assert!(
    serde_json::to_string(&report)
      .unwrap()
      .contains("\"cost_usd\":0.25")
  );
  let zero = Usage {
    cost_usd: Cost::usd(0.0),
    ..Usage::default()
  };
  let zero = Report::reviewed(Effort::Small, Vec::new(), Coverage::default(), zero);
  assert_eq!(
    serde_json::to_value(&zero).unwrap()["usage"]["cost_usd"],
    0.0
  );
  let unknown = Report::reviewed(
    Effort::Small,
    Vec::new(),
    Coverage::default(),
    Usage::default(),
  );
  assert_eq!(
    serde_json::to_value(&unknown).unwrap()["usage"]["cost_usd"],
    serde_json::Value::Null
  );
}

#[test]
fn contract_redaction_spans_serialize_only_when_present() {
  let plain = serde_json::to_value(finding(Severity::Low)).unwrap();
  assert!(plain.get("redaction_spans").is_none());
  let secret = Finding {
    source: Source::Secrets,
    redaction_spans: vec![RedactionSpan { start: 4, end: 9 }],
    ..finding(Severity::Blocker)
  };
  let doc = serde_json::to_value(&secret).unwrap();
  assert_eq!(
    doc["redaction_spans"],
    serde_json::json!([{ "start": 4, "end": 9 }])
  );
  assert_eq!(doc["source"], "secrets");
}

#[test]
fn a_review_is_complete_only_without_an_error_or_an_unreviewed_path() {
  let whole = reviewed(vec![finding(Severity::Low)], Coverage::default());
  assert!(whole.is_complete());
  let partial = reviewed(
    Vec::new(),
    Coverage {
      unreviewed: vec![Omitted {
        path: "src/lib.rs".to_owned(),
        reason: "lost".to_owned(),
      }],
      ..Coverage::default()
    },
  );
  assert!(!partial.is_complete());
  assert!(!Report::failed(Effort::Medium, ErrorKind::Outage, "down").is_complete());
}
