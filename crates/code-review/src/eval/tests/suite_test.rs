use super::{CaseReport, SCHEMA, SuiteReport, Verdict};
use crate::budget::Effort;
use crate::contract::{Coverage, Finding, Severity, Side, Source};
use crate::eval::{
  Arm, ArmReport, Class, EvalReport, Expectation, Kind, Revisions, Run, Setting, match_findings,
};

fn arm(setting: Setting, found: bool) -> ArmReport {
  let rows = [Expectation {
    id: "d".to_owned(),
    path: "src/lib.rs".to_owned(),
    claim: "underflow".to_owned(),
    kind: Kind::Defect,
  }];
  let findings = if found {
    vec![Finding {
      id: "f1".to_owned(),
      path: "src/lib.rs".to_owned(),
      line: 1,
      end_line: 1,
      side: Side::New,
      severity: Severity::High,
      category: "correctness".to_owned(),
      source: Source::Llm,
      claim: "slice underflow".to_owned(),
      quote: "q".to_owned(),
      suggestion: None,
      redaction_spans: Vec::new(),
    }]
  } else {
    Vec::new()
  };
  ArmReport::new(
    Run {
      arm: Arm::Baseline,
      setting,
      effort: Effort::Small,
      coverage: Coverage::default(),
      calls: Vec::new(),
      runtime_ms: 1,
    },
    &findings,
    match_findings(&rows, &findings),
  )
}

fn case(arms: Vec<ArmReport>, unverified: &[&str], gate: &[&str]) -> CaseReport {
  CaseReport {
    id: "c".to_owned(),
    class: Class::Supported,
    report: EvalReport::new(
      Revisions {
        base: "b".to_owned(),
        tip: "t".to_owned(),
      },
      arms,
    ),
    unverified: unverified.iter().map(|why| (*why).to_owned()).collect(),
    gate_violations: gate.iter().map(|why| (*why).to_owned()).collect(),
  }
}

fn suite(cases: Vec<CaseReport>) -> SuiteReport {
  SuiteReport {
    schema: SCHEMA,
    cases,
    calibration: None,
  }
}

#[test]
fn caught_defects_satisfy_and_a_miss_or_a_gate_violation_fails() {
  let ok = suite(vec![case(vec![arm(Setting::Small, true)], &[], &[])]);
  assert_eq!(ok.verdict(), Verdict::Satisfied);
  let missed = suite(vec![case(vec![arm(Setting::Small, false)], &[], &[])]);
  assert_eq!(missed.verdict(), Verdict::Missed);
  let gate = suite(vec![case(
    vec![arm(Setting::Small, true)],
    &[],
    &["advisory"],
  )]);
  assert_eq!(gate.verdict(), Verdict::Missed);
}

#[test]
fn the_known_weak_baseline_may_miss_without_failing_the_run() {
  let weak = suite(vec![case(
    vec![arm(Setting::Small, true), arm(Setting::None, false)],
    &[],
    &[],
  )]);
  assert_eq!(weak.verdict(), Verdict::Satisfied);
}

#[test]
fn anything_unverified_outranks_a_miss_and_is_never_a_pass() {
  let both = suite(vec![
    case(vec![arm(Setting::Small, false)], &[], &[]),
    case(
      Vec::new(),
      &["Baseline at small: replay miss: review-ab"],
      &[],
    ),
  ]);
  assert_eq!(both.verdict(), Verdict::Unverified);
  let text = both.summary();
  assert!(
    text.contains("c: unverified: Baseline at small: replay miss: review-ab"),
    "{text}"
  );
  assert!(
    text.contains("c Baseline/small caught 0 missed 1"),
    "{text}"
  );
}

#[test]
fn the_document_names_its_schema_and_omits_an_absent_calibration() {
  let doc = serde_json::to_value(suite(vec![case(Vec::new(), &[], &[])])).expect("json");
  assert_eq!(doc["schema"], 1);
  assert_eq!(doc["cases"][0]["id"], "c");
  assert_eq!(doc["cases"][0]["revisions"]["tip"], "t");
  assert_eq!(doc["cases"][0]["class"], "supported");
  assert!(doc.get("calibration").is_none());
}

#[test]
fn a_run_that_produced_no_arm_is_unverified_never_satisfied() {
  let empty = suite(vec![case(Vec::new(), &[], &[])]);
  assert_eq!(empty.verdict(), Verdict::Unverified);
  assert!(
    empty.summary().contains("c: unverified: no arm ran"),
    "{}",
    empty.summary()
  );
}
