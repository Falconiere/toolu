use super::{CaseReport, SCHEMA, SuiteReport, Verdict};
use crate::budget::Effort;
use crate::contract::{Coverage, Finding, Severity, Side, Source};
use crate::eval::{
  Arm, ArmReport, Class, EvalReport, Expectation, Kind, Revisions, Run, Setting, match_findings,
};

fn arm_from(setting: Setting, rows: &[Expectation], findings: &[Finding]) -> ArmReport {
  ArmReport::new(
    Run {
      arm: Arm::Baseline,
      setting,
      effort: Effort::Small,
      coverage: Coverage::default(),
      calls: Vec::new(),
      runtime_ms: 1,
    },
    findings,
    match_findings(rows, findings),
  )
}

fn arm(setting: Setting, found: bool) -> ArmReport {
  let rows = [Expectation {
    id: "d".to_owned(),
    path: "src/lib.rs".to_owned(),
    claim: "underflow".to_owned(),
    kind: Kind::Defect,
  }];
  let findings = if found {
    vec![sample_finding(
      "f1",
      1,
      Severity::High,
      "correctness",
      "slice underflow",
    )]
  } else {
    Vec::new()
  };
  arm_from(setting, &rows, &findings)
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

fn sample_finding(id: &str, line: u32, severity: Severity, category: &str, claim: &str) -> Finding {
  Finding {
    id: id.to_owned(),
    path: "src/lib.rs".to_owned(),
    line,
    end_line: line,
    side: Side::New,
    severity,
    category: category.to_owned(),
    source: Source::Llm,
    claim: claim.to_owned(),
    quote: "q".to_owned(),
    suggestion: None,
    redaction_spans: Vec::new(),
    assessment: None,
  }
}

fn arm_flagged_fp(setting: Setting) -> ArmReport {
  let rows = [
    Expectation {
      id: "d".to_owned(),
      path: "src/lib.rs".to_owned(),
      claim: "underflow".to_owned(),
      kind: Kind::Defect,
    },
    Expectation {
      id: "fp".to_owned(),
      path: "src/lib.rs".to_owned(),
      claim: "prefer rewrite".to_owned(),
      kind: Kind::FalsePositive,
    },
  ];
  let findings = vec![
    sample_finding("f1", 1, Severity::High, "correctness", "slice underflow"),
    sample_finding(
      "f2",
      2,
      Severity::Low,
      "style",
      "prefer rewrite in iterator style",
    ),
  ];
  arm_from(setting, &rows, &findings)
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
fn a_flagged_false_positive_on_a_non_none_arm_fails_the_suite() {
  let flagged = suite(vec![case(vec![arm_flagged_fp(Setting::Small)], &[], &[])]);
  assert_eq!(flagged.verdict(), Verdict::Missed);
  let weak_fp = suite(vec![case(
    vec![arm(Setting::Small, true), arm_flagged_fp(Setting::None)],
    &[],
    &[],
  )]);
  assert_eq!(weak_fp.verdict(), Verdict::Satisfied);
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
fn a_case_where_only_the_known_weak_baseline_ran_is_unverified() {
  let weak_only = suite(vec![case(vec![arm(Setting::None, false)], &[], &[])]);
  assert_eq!(weak_only.verdict(), Verdict::Unverified);
  assert!(
    weak_only
      .summary()
      .contains("c: unverified: no arm ran beyond the known-weak none baseline"),
    "{}",
    weak_only.summary()
  );
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
