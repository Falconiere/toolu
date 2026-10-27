//! The report contract, black-box: a full report validates against
//! `$defs/codeReview` of the CLI schema, and the memory records round-trip.

use serde_json::{Value, json};
use toolu_code_review::budget::{
  CHECKLIST_MAX_BYTES, Effort, HTTP_RESPONSE_MAX_BYTES, JEV_REQUEST_MAX_BYTES, JEV_STATE_MAX_BYTES,
  RULES_MAX_BYTES,
};
use toolu_code_review::contract::{
  Coverage, ErrorKind, Finding, Omitted, Report, ReportError, Severity, Side, Source, Usage,
  Verdict,
};
use toolu_code_review::memory::{Counts, Dismissal, Pass, PriorFinding, Status};

type Res<T> = Result<T, String>;

/// The errors of `document` against `$defs/codeReview`.
fn violations(document: &Value) -> Res<Vec<String>> {
  let path = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../cli/src/commands.schema.json"
  );
  let text = std::fs::read_to_string(path).map_err(|err| err.to_string())?;
  let schema: Value = serde_json::from_str(&text).map_err(|err| err.to_string())?;
  let schema = json!({
    "$schema": schema.get("$schema"),
    "$defs": schema.get("$defs"),
    "$ref": "#/$defs/codeReview",
  });
  let validator = jsonschema::validator_for(&schema).map_err(|err| err.to_string())?;
  Ok(
    validator
      .iter_errors(document)
      .map(|err| err.to_string())
      .collect(),
  )
}

fn finding(id: &str, severity: Severity, side: Side, suggestion: Option<&str>) -> Finding {
  Finding {
    id: id.to_owned(),
    path: "src/lib.rs".to_owned(),
    line: 10,
    end_line: 12,
    side,
    severity,
    category: "correctness".to_owned(),
    source: Source::Llm,
    claim: "The slice underflows when len is 0.".to_owned(),
    quote: "let last = items[items.len() - 1];".to_owned(),
    suggestion: suggestion.map(str::to_owned),
  }
}

fn blocked_report() -> Report {
  let mut report = Report::failed(Effort::Medium, ErrorKind::Outage, "unused");
  report.verdict = Verdict::Blocked;
  report.error = None;
  report.findings = vec![
    finding(
      "f1",
      Severity::Blocker,
      Side::New,
      Some("Use items.last()."),
    ),
    finding("f2", Severity::Medium, Side::Old, None),
    finding("f3", Severity::Low, Side::New, None),
    Finding {
      source: Source::AstGrep,
      ..finding("f4", Severity::Nit, Side::New, None)
    },
    Finding {
      source: Source::Secrets,
      ..finding("f5", Severity::High, Side::New, None)
    },
  ];
  report.coverage = Coverage {
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
  report.usage = Usage {
    tokens_in: 1200,
    tokens_out: 3400,
    cost_usd: 0.0123,
    model: Some("deepseek/deepseek-v4-pro-0813".to_owned()),
  };
  report
}

#[test]
fn a_blocked_report_validates_and_counts_blocking_findings() {
  let report = blocked_report();
  assert_eq!(report.blocking_count(), 3);
  let doc = serde_json::to_value(&report).unwrap();
  assert_eq!(violations(&doc).unwrap(), Vec::<String>::new());
  assert_eq!(doc["findings"][0]["suggestion"], "Use items.last().");
  assert!(doc["findings"][1].get("suggestion").is_none());
  assert_eq!(doc["findings"][1]["side"], "old");
  assert_eq!(doc["findings"][3]["source"], "ast-grep");
  assert_eq!(doc["error"], Value::Null);
}

#[test]
fn a_failed_report_validates_and_a_bad_severity_does_not() {
  for effort in Effort::ALL {
    let report = Report::failed(effort, ErrorKind::Unconfigured, "no key");
    let error = ReportError {
      kind: ErrorKind::Unconfigured,
      message: "no key".to_owned(),
    };
    assert_eq!(report.error.as_ref(), Some(&error));
    let doc = serde_json::to_value(&report).unwrap();
    assert_eq!(violations(&doc).unwrap(), Vec::<String>::new());
  }
  let mut doc = serde_json::to_value(blocked_report()).unwrap();
  doc["findings"][0]["severity"] = json!("critical");
  assert_ne!(violations(&doc).unwrap(), Vec::<String>::new());
}

#[test]
fn the_memory_records_round_trip() {
  let prior = PriorFinding {
    fingerprint: "src/lib.rs\u{1f}correctness\u{1f}slice underflows".to_owned(),
    id: "f1".to_owned(),
    severity: Severity::Blocker,
    status: Status::Dismissed,
  };
  let dismissal = Dismissal {
    id: "f1".to_owned(),
    reason: "guarded by the caller".to_owned(),
    by: "agent".to_owned(),
  };
  let pass = Pass {
    diff_sha: "abc123".to_owned(),
    verdict: Verdict::Blocked,
    counts: Counts {
      blocking: 3,
      advisory: 2,
    },
  };
  let text = serde_json::to_string(&(&prior, &dismissal, &pass)).unwrap();
  assert!(
    text.contains("\"dismissed\"") && text.contains("\"blocked\""),
    "{text}"
  );
  let back: (PriorFinding, Dismissal, Pass) = serde_json::from_str(&text).unwrap();
  assert_eq!(back, (prior, dismissal, pass));
  for status in [Status::Open, Status::Resolved] {
    assert_ne!(serde_json::to_string(&status).unwrap(), "\"dismissed\"");
  }
}

#[test]
fn the_fixed_caps_are_the_starting_values() {
  assert_eq!(RULES_MAX_BYTES, 65_536);
  assert_eq!(CHECKLIST_MAX_BYTES, 4_506);
  assert_eq!(
    (JEV_STATE_MAX_BYTES, JEV_REQUEST_MAX_BYTES),
    (90_000, 180_000)
  );
  assert_eq!(HTTP_RESPONSE_MAX_BYTES, 1_048_576);
  assert_eq!(
    Counts::default(),
    Counts {
      blocking: 0,
      advisory: 0
    }
  );
}
