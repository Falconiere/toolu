use super::{
  Arm, ArmReport, CallRecord, EvalReport, FindingsSummary, LiveOrReplayed, Revisions, Run,
};
use crate::budget::Effort;
use crate::contract::{Coverage, Finding, Severity, Side, Source};
use crate::eval::{Expectation, Kind, match_findings};
use crate::review::{Call, Stage};

fn finding(id: &str, claim: &str, severity: Severity) -> Finding {
  Finding {
    id: id.to_owned(),
    path: "src/lib.rs".to_owned(),
    line: 3,
    end_line: 3,
    side: Side::New,
    severity,
    category: "correctness".to_owned(),
    source: Source::Llm,
    claim: claim.to_owned(),
    quote: "quote".to_owned(),
    suggestion: None,
    redaction_spans: Vec::new(),
  }
}

fn call(stage: Stage, tokens_in: u64, tokens_out: u64, cost: Option<f64>) -> Call {
  Call {
    stage,
    model: "m".to_owned(),
    provider: Some("p".to_owned()),
    quantization: None,
    tokens_in,
    tokens_out,
    cost_usd: cost,
    millis: 40,
    outcome: "ok".to_owned(),
  }
}

fn arm(calls: Vec<CallRecord>, findings: &[Finding]) -> ArmReport {
  let rows = [Expectation {
    id: "d".to_owned(),
    path: "src/lib.rs".to_owned(),
    claim: "underflow".to_owned(),
    kind: Kind::Defect,
  }];
  let outcome = match_findings(&rows, findings);
  ArmReport::new(
    Run {
      arm: Arm::Baseline,
      effort: Effort::Medium,
      coverage: Coverage::default(),
      calls,
      runtime_ms: 120,
    },
    findings,
    outcome,
  )
}

#[test]
fn a_replayed_call_keeps_its_tokens_but_bills_nothing() {
  let live = CallRecord::from_call(
    &call(Stage::Review, 100, 50, Some(0.25)),
    LiveOrReplayed::Live,
  );
  let replayed = CallRecord::from_call(
    &call(Stage::Review, 100, 50, Some(0.25)),
    LiveOrReplayed::Replayed,
  );
  assert_eq!((live.tokens, live.cost), (150, Some(0.25)));
  assert_eq!((replayed.tokens, replayed.cost), (150, Some(0.0)));
  assert_eq!(replayed.live_or_replayed, LiveOrReplayed::Replayed);
}

#[test]
fn usage_sums_every_call_but_bills_only_live_ones() {
  let calls = vec![
    CallRecord::from_call(
      &call(Stage::Review, 100, 50, Some(0.5)),
      LiveOrReplayed::Live,
    ),
    CallRecord::from_call(
      &call(Stage::Judge, 10, 5, Some(0.5)),
      LiveOrReplayed::Replayed,
    ),
    CallRecord::from_call(&call(Stage::Selector, 1, 1, None), LiveOrReplayed::Live),
  ];
  let report = arm(calls, &[]);
  assert_eq!(report.usage.tokens_in, 111);
  assert_eq!(report.usage.tokens_out, 56);
  assert!((report.usage.cost_usd - 0.5).abs() < f64::EPSILON);
  assert_eq!(report.runtime_ms, 120);
}

#[test]
fn tokens_per_finding_divides_total_tokens_and_is_absent_without_findings() {
  let calls = || {
    vec![CallRecord::from_call(
      &call(Stage::Review, 300, 100, None),
      LiveOrReplayed::Live,
    )]
  };
  let findings = [
    finding("f1", "slice underflows", Severity::High),
    finding("f2", "style", Severity::Low),
  ];
  let some = arm(calls(), &findings);
  assert_eq!(some.tokens_per_finding, Some(200.0));
  assert_eq!(arm(calls(), &[]).tokens_per_finding, None);
}

#[test]
fn findings_summary_counts_blocking_severities() {
  let findings = [
    finding("f1", "a", Severity::Blocker),
    finding("f2", "b", Severity::Medium),
    finding("f3", "c", Severity::Low),
    finding("f4", "d", Severity::Nit),
  ];
  let summary = FindingsSummary::of(&findings);
  assert_eq!((summary.total, summary.blocking), (4, 2));
}

#[test]
fn the_document_serializes_origin_matches_and_unclassified() {
  let findings = [
    finding("f1", "slice underflow here", Severity::High),
    finding("f2", "leftover", Severity::Low),
  ];
  let calls = vec![CallRecord::from_call(
    &call(Stage::Review, 10, 10, Some(0.1)),
    LiveOrReplayed::Replayed,
  )];
  let report = EvalReport::new(
    Revisions {
      base: "aaa".to_owned(),
      tip: "bbb".to_owned(),
    },
    vec![arm(calls, &findings)],
  );
  let doc = serde_json::to_value(&report).unwrap();
  assert_eq!(doc["schema"], 1);
  assert_eq!(doc["revisions"]["tip"], "bbb");
  let first = &doc["arms"][0];
  assert_eq!(first["arm"], "baseline");
  assert_eq!(first["calls"][0]["live_or_replayed"], "replayed");
  assert_eq!(first["calls"][0]["tokens"], 20);
  assert_eq!(first["calls"][0]["stage"], "review");
  assert_eq!(first["matches"][0]["status"], "caught");
  assert_eq!(first["unclassified"][0]["id"], "f2");
  assert_eq!(first["tokens_per_finding"], 10.0);
  assert!(first["calls"][0].get("tokens_in").is_none());
}
