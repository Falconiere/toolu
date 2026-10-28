use std::path::PathBuf;

use super::{arm_report, prepare, run_case};
use crate::budget::Effort;
use crate::contract::{Omitted, Usage};
use crate::eval::{Arm, ArmChoice, Fixtures, Mode, Plan, Setting, Store};
use crate::pipeline;
use crate::prepass::{Notice, NoticeKind};
use crate::review::{EffortChoice, EffortSource, Reviewed};
use toolu_runtime::env::Env;

fn golden() -> Fixtures {
  let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/code-review-eval");
  Fixtures::load(&root).unwrap()
}

/// A replay-only plan over a temporary empty store.
fn replay_plan<'a>(
  env: &'a Env,
  mode: &'a Mode,
  arms: ArmChoice,
  settings: &'a [Setting],
) -> Plan<'a> {
  Plan {
    env,
    arms,
    settings,
    mode,
    openrouter: toolu_openrouter::Config::default(),
    jev: None,
  }
}

#[test]
fn enhanced_at_the_known_weak_none_setting_runs_no_arm() {
  let fixtures = golden();
  let empty = tempfile::tempdir().unwrap();
  let mode = Mode::Replay(Store::new(empty.path()));
  let env = Env::from_pairs([("GIT_CONFIG_NOSYSTEM", "1")]);
  let plan = replay_plan(&env, &mode, ArmChoice::Enhanced, &[Setting::None]);
  let case = fixtures
    .set()
    .cases
    .iter()
    .find(|case| case.id == "slice-underflow")
    .unwrap();
  let report = run_case(&fixtures, case, &plan).unwrap();
  assert_eq!(report.report.arms.len(), 0);
  assert_eq!(report.unverified.len(), 1);
  assert!(
    report.unverified[0].contains("no arm selected") && report.unverified[0].contains("Enhanced"),
    "{}",
    report.unverified[0]
  );
}

fn reviewed() -> Reviewed {
  Reviewed {
    effort: EffortChoice {
      level: Effort::Small,
      source: EffortSource::Flag,
      floor: None,
      reason: "test".to_owned(),
    },
    findings: Vec::new(),
    dismissed: Vec::new(),
    unreviewed: Vec::new(),
    notes: Vec::new(),
    usage: Usage::default(),
    calls: Vec::new(),
  }
}

/// A silent, whole review of `slice-underflow` with `notices` from the pre-pass.
fn silent_arm(notices: Vec<Notice>) -> Result<(), String> {
  let fixtures = golden();
  let empty = tempfile::tempdir().map_err(|err| err.to_string())?;
  let mode = Mode::Replay(Store::new(empty.path()));
  let env = Env::from_pairs([("GIT_CONFIG_NOSYSTEM", "1")]);
  let plan = replay_plan(&env, &mode, ArmChoice::Baseline, &[Setting::Small]);
  let case = fixtures
    .set()
    .case("slice-underflow")
    .ok_or("missing case")?;
  let (_repo, mut prepared) = prepare(&fixtures, case, &plan)?;
  prepared.prepass.notices = notices;
  prepared.prepass.overflow = false;
  arm_report(
    &prepared,
    &plan,
    case,
    (Setting::Small, Arm::Baseline, 1),
    reviewed(),
  )
  .map(|_| ())
}

#[test]
fn a_degraded_prepass_leaves_the_arm_unverified_as_production_would() {
  let missing = Notice {
    kind: NoticeKind::MissingAstGrep,
    text: "ast-grep not found".to_owned(),
  };
  let why = silent_arm(vec![missing]).unwrap_err();
  assert!(
    why.contains("incomplete review") && why.contains("mechanical checks degraded"),
    "{why}"
  );
  let failed = Notice {
    kind: NoticeKind::AstGrepFailed,
    text: "scanner failed".to_owned(),
  };
  assert!(silent_arm(vec![failed]).is_err());
}

#[test]
fn a_prepass_without_degradation_still_scores_the_arm() {
  silent_arm(Vec::new()).unwrap();
}

#[test]
fn an_unreexamined_prior_in_the_diff_stays_open_and_blocks_coverage() {
  let fixtures = golden();
  let empty = tempfile::tempdir().unwrap();
  let mode = Mode::Replay(Store::new(empty.path()));
  let env = Env::from_pairs([("GIT_CONFIG_NOSYSTEM", "1")]);
  let plan = replay_plan(&env, &mode, ArmChoice::Baseline, &[Setting::Small]);
  let case = fixtures.set().case("memory-unreexamined").unwrap();
  let (_repo, prepared) = prepare(&fixtures, case, &plan).unwrap();
  assert!(
    prepared
      .shaped
      .gate_files
      .iter()
      .any(|path| path == "src/legacy.rs")
  );
  // The review lost the prior's file, so the prior was not re-examined.
  let mut lost = reviewed();
  lost.unreviewed.push(Omitted {
    path: "src/legacy.rs".to_owned(),
    reason: "review call failed".to_owned(),
  });
  let (report, recap) =
    pipeline::completed(&prepared.shaped, &prepared.prepass, &prepared.prior, lost);
  assert_eq!(recap.still_open, ["mem-legacy-1"]);
  assert_eq!(recap.resolved, Vec::<String>::new());
  assert!(
    report.coverage.unreviewed.iter().any(|item| {
      item.path == "src/legacy.rs"
        && item.reason == "prior finding mem-legacy-1 was not re-examined"
    }),
    "{:?}",
    report.coverage.unreviewed
  );
  assert!(!report.is_complete());
}
