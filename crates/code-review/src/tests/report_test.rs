use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Ctx;

use super::{present, summary};
use crate::budget::Effort;
use crate::contract::{Coverage, Omitted, Report, Usage};

#[test]
fn compact_summary_omits_empty_sections_and_names_unreviewed() {
  let report = Report::reviewed(
    Effort::Small,
    Vec::new(),
    Coverage::default(),
    Usage::default(),
  );
  let text = summary(&report);
  assert!(text.starts_with("- [x] code-review: approved · 0 blocking · effort: small"));
  assert!(!text.contains("Findings") && !text.contains("Mechanical checks"));
  let mut partial = report;
  partial.coverage.unreviewed.push(Omitted {
    path: "src/big.rs".into(),
    reason: "over budget".into(),
  });
  let outcome = present(&partial, &Ctx::default());
  assert_eq!(outcome.exit, Exit::Blocked);
  assert!(outcome.stdout.unwrap().contains("src/big.rs: over budget"));
}

#[test]
fn a_judged_finding_shows_its_assessment_and_a_deterministic_one_shows_none() {
  use crate::contract::{Assessment, Finding, Severity, Side, Source};
  let finding = |source, assessment| Finding {
    id: "f".into(),
    path: "src/a.rs".into(),
    line: 3,
    end_line: 3,
    side: Side::New,
    severity: Severity::High,
    category: "correctness".into(),
    source,
    claim: "the index can underflow".into(),
    quote: "items[i - 1]".into(),
    suggestion: None,
    redaction_spans: Vec::new(),
    assessment,
  };
  let findings = vec![
    finding(Source::Llm, Some(Assessment::Supported)),
    finding(Source::AstGrep, None),
  ];
  let report = Report::reviewed(
    Effort::High,
    findings,
    Coverage::default(),
    Usage::default(),
  );
  let text = summary(&report);
  assert!(
    text.contains("[high, jev: supported] src/a.rs:3 the index"),
    "{text}"
  );
  assert!(text.contains("[high] src/a.rs:3 the index"), "{text}");
  let json = serde_json::to_value(&report).unwrap();
  assert_eq!(json["findings"][0]["assessment"], "supported");
  assert!(json["findings"][1].get("assessment").is_none());
}
