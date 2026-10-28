use super::{AutoChoice, calibrate};
use crate::budget::Effort;
use crate::contract::{Coverage, Finding, Severity, Side, Source};
use crate::eval::{
  Arm, ArmReport, CallRecord, Expectation, Kind, LiveOrReplayed, Run, Setting, match_findings,
};
use crate::review::{Call, Stage};

fn row(id: &str, claim: &str, kind: Kind) -> Expectation {
  Expectation {
    id: id.to_owned(),
    path: "src/lib.rs".to_owned(),
    claim: claim.to_owned(),
    kind,
  }
}

fn finding(claim: &str) -> Finding {
  Finding {
    id: claim.to_owned(),
    path: "src/lib.rs".to_owned(),
    line: 1,
    end_line: 1,
    side: Side::New,
    severity: Severity::High,
    category: "correctness".to_owned(),
    source: Source::Llm,
    claim: claim.to_owned(),
    quote: "q".to_owned(),
    suggestion: None,
    redaction_spans: Vec::new(),
    assessment: None,
  }
}

fn call(tokens_in: u64, tokens_out: u64, cost: f64) -> CallRecord {
  let call = Call {
    stage: Stage::Review,
    model: "m".to_owned(),
    provider: None,
    quantization: None,
    tokens_in,
    tokens_out,
    cost_usd: Some(cost),
    millis: 1,
    outcome: "ok".to_owned(),
  };
  CallRecord::from_call(&call, LiveOrReplayed::Live)
}

/// An arm that raised `claims`, judged against one defect row and one false-positive row.
fn arm(setting: Setting, effort: Effort, claims: &[&str], tokens: u64, cost: f64) -> ArmReport {
  let rows = [
    row("defect", "underflow", Kind::Defect),
    row("fp", "rewrite", Kind::FalsePositive),
  ];
  let findings: Vec<Finding> = claims.iter().map(|claim| finding(claim)).collect();
  ArmReport::new(
    Run {
      arm: Arm::Enhanced,
      setting,
      effort,
      coverage: Coverage::default(),
      calls: vec![call(tokens, 0, cost)],
      runtime_ms: 1,
    },
    &findings,
    match_findings(&rows, &findings),
  )
}

fn level(effort: Effort, claims: &[&str]) -> ArmReport {
  let setting = Setting::parse(effort.name()).expect("a fixed level is a setting");
  arm(setting, effort, claims, 100, 0.5)
}

#[test]
fn totals_sum_defects_false_positives_tokens_and_cost_per_setting() {
  let first = [
    level(Effort::Small, &["rewrite it"]),
    level(Effort::Medium, &["slice underflow", "prefer rewrite"]),
  ];
  let second = [
    level(Effort::Small, &["slice underflow"]),
    level(Effort::Medium, &["slice underflow"]),
  ];
  let out = calibrate(&[("one", &first), ("two", &second)]);
  let small = &out.settings[0];
  assert_eq!(small.setting, Setting::Small);
  assert_eq!(
    (small.cases, small.defects_caught, small.defects_missed),
    (2, 1, 1)
  );
  assert_eq!(small.false_positives, 1);
  assert_eq!(small.tokens, 200);
  assert!((small.cost_usd - 1.0).abs() < f64::EPSILON);
  let medium = &out.settings[1];
  assert_eq!(
    (
      medium.defects_caught,
      medium.defects_missed,
      medium.false_positives
    ),
    (2, 0, 1)
  );
  assert_eq!(out.auto, Vec::<AutoChoice>::new());
  assert_eq!(out.auto_below_lowest, 0);
}

#[test]
fn auto_below_the_lowest_level_that_caught_every_defect_is_counted() {
  // Small misses; medium is the lowest level that catches everything.
  let low = [
    level(Effort::Small, &[]),
    level(Effort::Medium, &["slice underflow"]),
    level(Effort::High, &["slice underflow"]),
    arm(Setting::Auto, Effort::Small, &[], 40, 0.1),
  ];
  // Small already catches the defect, so medium is not below anything.
  let fine = [
    level(Effort::Small, &["slice underflow"]),
    level(Effort::Xhigh, &["slice underflow"]),
    arm(Setting::Auto, Effort::Medium, &["slice underflow"], 40, 0.1),
  ];
  // No fixed level catches it: nothing to be below.
  let none = [
    level(Effort::Small, &[]),
    arm(Setting::Auto, Effort::Small, &[], 40, 0.1),
    arm(Setting::None, Effort::Small, &[], 10, 0.0),
  ];
  let out = calibrate(&[("low", &low), ("fine", &fine), ("none", &none)]);
  assert_eq!(
    out.auto,
    [
      AutoChoice {
        case: "low".to_owned(),
        arm: Arm::Enhanced,
        chose: Effort::Small,
        lowest_catching: Some(Effort::Medium),
        below: true,
      },
      AutoChoice {
        case: "fine".to_owned(),
        arm: Arm::Enhanced,
        chose: Effort::Medium,
        lowest_catching: Some(Effort::Small),
        below: false,
      },
      AutoChoice {
        case: "none".to_owned(),
        arm: Arm::Enhanced,
        chose: Effort::Small,
        lowest_catching: None,
        below: false,
      },
    ]
  );
  assert_eq!(out.auto_below_lowest, 1);
  let weak = out
    .settings
    .iter()
    .find(|row| row.setting == Setting::None)
    .expect("none is aggregated");
  assert_eq!((weak.cases, weak.defects_missed, weak.tokens), (1, 1, 10));
}

#[test]
fn an_auto_run_that_caught_every_defect_is_never_too_low() {
  // Auto chose small and caught the defect; small was not run as a fixed level.
  let caught = [
    level(Effort::Medium, &["slice underflow"]),
    arm(Setting::Auto, Effort::Small, &["slice underflow"], 40, 0.1),
  ];
  let out = calibrate(&[("caught", &caught)]);
  assert_eq!(out.auto.len(), 1);
  assert_eq!(out.auto[0].lowest_catching, Some(Effort::Medium));
  assert!(!out.auto[0].below);
  assert_eq!(out.auto_below_lowest, 0);
}

#[test]
fn an_auto_run_with_no_fixed_level_beside_it_is_not_measured() {
  let arms = [arm(Setting::Auto, Effort::Medium, &[], 40, 0.1)];
  let out = calibrate(&[("alone", &arms)]);
  assert_eq!(out.auto, Vec::<AutoChoice>::new());
  assert_eq!(out.settings.len(), 1);
}
