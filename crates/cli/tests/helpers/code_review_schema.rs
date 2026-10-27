//! `toolu code-review` reports against `$defs/codeReview` (#661): the binary's
//! fail-closed document, and full reports built through the contract.

use serde_json::{Value, json};
use toolu_code_review::budget::Effort;
use toolu_code_review::contract::{
  Cost, Coverage, ErrorKind, Finding, Omitted, Report, ReportError, Severity, Side, Source, Usage,
};

use crate::cli::{one_document, stderr, toolu};
use crate::schema::violations;

fn finding(id: &str, severity: Severity, side: Side, source: Source) -> Finding {
  Finding {
    id: id.to_owned(),
    path: "src/lib.rs".to_owned(),
    line: 10,
    end_line: 12,
    side,
    severity,
    category: "correctness".to_owned(),
    source,
    claim: "The slice underflows when len is 0.".to_owned(),
    quote: "let last = items[items.len() - 1];".to_owned(),
    suggestion: None,
  }
}

/// A reviewed run with every severity, side and source, and every coverage list.
fn blocked_report() -> Report {
  let findings = vec![
    Finding {
      suggestion: Some("Use items.last().".to_owned()),
      ..finding("f1", Severity::Blocker, Side::New, Source::Llm)
    },
    finding("f2", Severity::Medium, Side::Old, Source::Llm),
    finding("f3", Severity::Low, Side::New, Source::Llm),
    finding("f4", Severity::Nit, Side::New, Source::AstGrep),
    finding("f5", Severity::High, Side::New, Source::Secrets),
  ];
  let coverage = Coverage {
    reviewed: vec!["src/lib.rs".to_owned()],
    unreviewed: vec![Omitted {
      path: "src/big.rs".to_owned(),
      reason: "over the per-file budget".to_owned(),
    }],
    skipped: vec![Omitted {
      path: "Cargo.lock".to_owned(),
      reason: "lockfile".to_owned(),
    }],
  };
  let usage = Usage {
    tokens_in: 1200,
    tokens_out: 3400,
    cost_usd: Cost::usd(0.0123).unwrap_or_default(),
    model: Some("deepseek/deepseek-v4-pro-0813".to_owned()),
  };
  Report::reviewed(Effort::Medium, findings, coverage, usage)
}

#[test]
fn the_binary_s_report_validates_and_exits_unavailable() {
  let output = toolu(&["--json", "code-review", "run", "--effort", "high"]).unwrap();
  assert_eq!(output.status.code(), Some(69));
  assert_eq!(stderr(&output).unwrap(), "");
  let document = one_document(&output).unwrap();
  assert_eq!(
    violations(&document, Some("codeReview")).unwrap(),
    Vec::<String>::new()
  );
  assert_eq!(
    (
      &document["verdict"],
      &document["effort"],
      &document["error"]["kind"]
    ),
    (&json!("error"), &json!("high"), &json!("unconfigured"))
  );
}

#[test]
fn a_blocked_report_validates_and_counts_blocking_findings() {
  let report = blocked_report();
  assert_eq!(
    (report.verdict().name(), report.blocking_count()),
    ("blocked", 3)
  );
  let doc = serde_json::to_value(&report).unwrap();
  assert_eq!(
    violations(&doc, Some("codeReview")).unwrap(),
    Vec::<String>::new()
  );
  let ids: Vec<&Value> = (0..5).map(|index| &doc["findings"][index]["id"]).collect();
  assert_eq!(ids, ["f1", "f5", "f2", "f3", "f4"], "worst first");
  assert_eq!(doc["findings"][0]["suggestion"], "Use items.last().");
  assert!(doc["findings"][1].get("suggestion").is_none());
  assert_eq!(doc["findings"][2]["side"], "old");
  assert_eq!(doc["findings"][4]["source"], "ast-grep");
  assert_eq!(doc["error"], Value::Null);
}

#[test]
fn every_failed_report_validates_and_a_bad_severity_does_not() {
  for effort in Effort::ALL {
    let report = Report::failed(effort, ErrorKind::Unconfigured, "no key");
    let error = ReportError {
      kind: ErrorKind::Unconfigured,
      message: "no key".to_owned(),
    };
    assert_eq!(report.error.as_ref(), Some(&error));
    let doc = serde_json::to_value(&report).unwrap();
    assert_eq!(
      violations(&doc, Some("codeReview")).unwrap(),
      Vec::<String>::new()
    );
  }
  let mut doc = serde_json::to_value(blocked_report()).unwrap();
  doc["findings"][0]["severity"] = json!("critical");
  assert_ne!(
    violations(&doc, Some("codeReview")).unwrap(),
    Vec::<String>::new()
  );
}

#[test]
fn error_is_set_exactly_when_the_verdict_is_error() {
  let mut failed =
    serde_json::to_value(Report::failed(Effort::Auto, ErrorKind::Outage, "down")).unwrap();
  failed["error"] = Value::Null;
  assert_ne!(
    violations(&failed, Some("codeReview")).unwrap(),
    Vec::<String>::new()
  );
  let mut blocked = serde_json::to_value(blocked_report()).unwrap();
  blocked["error"] = json!({ "kind": "outage", "message": "down" });
  assert_ne!(
    violations(&blocked, Some("codeReview")).unwrap(),
    Vec::<String>::new()
  );
}
