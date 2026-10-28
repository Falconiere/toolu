//! The replay transport of the eval runner, end to end: a live run against a
//! loopback origin records its exchange; a replay run with no key and no route to
//! any origin answers the same request from the store at no charge; and a request
//! the store lacks fails the arm closed as unverified.

use std::path::Path;
use std::time::Duration;

use serde_json::json;
use toolu_code_review::eval::calibration::{Calibration, SettingTotals};
use toolu_code_review::eval::expectations::SET_VERSION;
use toolu_code_review::eval::suite::SuiteReport;
use toolu_code_review::eval::{
  ArmChoice, ArmReport, CaseReport, Fixtures, LiveOrReplayed, Mode, Plan, Setting, Store, Tally,
  UsageTotals, Verdict, run_all, run_case,
};
use toolu_code_review::review::Stage;
use toolu_http_test_support::{Fixture, Reply};
use toolu_runtime::env::Env;

use super::{case, golden};

pub(super) const REVIEW: &str = "/api/v1/chat/completions";
pub(super) const JEV: &str = "/api/v1/systemone";
const KEY: &str = "private-sentinel-670";

/// The environment of a run without a key: Git's isolation and the process's paths.
pub(super) fn keyless() -> Env {
  let mut env = Env::from_pairs([
    ("GIT_CONFIG_NOSYSTEM", "1".to_owned()),
    ("GIT_CONFIG_GLOBAL", "/dev/null".to_owned()),
    ("XDG_CONFIG_HOME", "/dev/null".to_owned()),
  ]);
  let process = Env::process();
  for name in ["PATH", "HOME", "TMPDIR"] {
    if let Some(value) = process.get(name) {
      env = env.with(name, value);
    }
  }
  env
}

/// A review answer naming the slice-underflow defect on its changed line.
pub(super) fn answer() -> String {
  let content = json!({"findings": [{
    "path": "src/lib.rs", "line": 2, "end_line": 2, "side": "new", "severity": "high",
    "category": "correctness", "claim": "The slice underflows when count exceeds the length",
    "quote": "let start = items.len() - count;", "suggestion": null, "prior_id": null
  }]});
  json!({
    "model": "deepseek/deepseek-v4-pro-0813",
    "provider": "Parasail",
    "choices": [{"message": {"content": content.to_string()}, "finish_reason": "stop"}],
    "usage": {"prompt_tokens": 100, "completion_tokens": 40, "cost": 0.01}
  })
  .to_string()
}

#[test]
fn a_replay_miss_fails_the_arm_closed_naming_the_missing_request() {
  let fixtures = golden().expect("fixture");
  let empty = tempfile::tempdir().expect("fs");
  let mode = Mode::Replay(Store::new(empty.path()));
  let plan = Plan {
    env: &keyless(),
    arms: ArmChoice::Baseline,
    settings: &[Setting::Small],
    mode: &mode,
    openrouter: toolu_openrouter::Config::default(),
    jev: None,
  };
  let underflow = case(&fixtures, "slice-underflow").expect("fixture");
  let report = run_case(&fixtures, underflow, &plan).expect("shapes");
  assert_eq!(report.report.arms.len(), 0);
  assert_eq!(report.unverified.len(), 1, "{:?}", report.unverified);
  let why = report.unverified.first().expect("one").as_str();
  assert!(
    why.contains("review-") && why.contains("Baseline at small"),
    "{why}"
  );

  let suite = run_all(&fixtures, &plan).expect("shapes");
  assert_eq!(suite.verdict(), Verdict::Unverified);
  assert!(suite.cases.iter().all(|case| !case.unverified.is_empty()));
  assert!(suite.summary().contains("unverified"));
  assert!(suite.calibration.is_none());

  // Several settings add the calibration section; `none` only ever runs the baseline.
  let many = Plan {
    arms: ArmChoice::Both,
    settings: &[Setting::Small, Setting::Auto, Setting::None],
    ..plan
  };
  let suite: SuiteReport = run_all(&fixtures, &many).expect("shapes");
  assert!(suite.cases.iter().all(|case| case.unverified.len() == 5));
  let document = serde_json::to_value(&suite).expect("json");
  assert_eq!(document["calibration"]["auto_below_lowest"], 0);
  let calibration: &Calibration = suite.calibration.as_ref().expect("several settings");
  let totals: &[SettingTotals] = &calibration.settings;
  assert_eq!(totals.len(), 0);
  assert_eq!(fixtures.set().version, SET_VERSION);
}

/// Jev's verdict on the one finding: supported.
fn judgment() -> String {
  json!({
    "model": "typesafe/jev-1.13",
    "usage": {"input_tokens": 30, "output_tokens": 3, "cost": 0.002},
    "answers": {"f0": {"type": "choice", "choice": "supported",
      "probabilities": {"supported": 0.9, "contradicted": 0.05, "insufficient": 0.05},
      "confidence": 0.9}}
  })
  .to_string()
}

fn endpoints(origin: &Fixture) -> (toolu_openrouter::Config, Option<toolu_jev_client::Config>) {
  let root = Some(origin.root_ca_der().to_vec());
  (
    toolu_openrouter::Config {
      endpoint: origin.url(REVIEW),
      pause: Duration::from_millis(10),
      test_root_ca_der: root.clone(),
    },
    Some(toolu_jev_client::Config {
      endpoint: origin.url(JEV),
      pause: Duration::from_millis(10),
      test_root_ca_der: root,
    }),
  )
}

/// A live run of both arms against the loopback origin, recording into `store`.
pub(super) fn record(
  fixtures: &Fixtures,
  origin: &Fixture,
  store: &Path,
) -> Result<CaseReport, String> {
  let env = keyless()
    .with("OPENROUTER_API_KEY", KEY)
    .with("OPENROUTER_MODEL", "")
    .with("JEV_TIMEOUT", "5")
    .with("HTTPS_PROXY", &origin.proxy_url())
    .with("NO_PROXY", "");
  let mode = Mode::Live(Some(Store::new(store)));
  let (openrouter, jev) = endpoints(origin);
  let plan = Plan {
    env: &env,
    arms: ArmChoice::Both,
    settings: &[Setting::Small],
    mode: &mode,
    openrouter,
    jev,
  };
  run_case(fixtures, case(fixtures, "slice-underflow")?, &plan)
}

/// A replay run of both arms: no key and the real endpoints, so only the store answers.
fn replay(fixtures: &Fixtures, store: &Path) -> Result<CaseReport, String> {
  let mode = Mode::Replay(Store::new(store));
  let plan = Plan {
    env: &keyless(),
    arms: ArmChoice::Both,
    settings: &[Setting::Small],
    mode: &mode,
    openrouter: toolu_openrouter::Config::default(),
    jev: None,
  };
  run_case(fixtures, case(fixtures, "slice-underflow")?, &plan)
}

fn file_names(dir: &Path) -> Result<Vec<String>, String> {
  let mut names = Vec::new();
  for entry in std::fs::read_dir(dir).map_err(|error| error.to_string())? {
    let entry = entry.map_err(|error| error.to_string())?;
    names.push(entry.file_name().to_string_lossy().into_owned());
  }
  names.sort();
  Ok(names)
}

/// The baseline bills the review call; the enhanced arm reuses that recorded answer
/// for the identical request and bills only Jev's judgment.
fn assert_billed_live(base: &ArmReport, enhanced: &ArmReport) {
  let origins = |arm: &ArmReport, stage: Stage| -> Vec<LiveOrReplayed> {
    arm
      .calls
      .iter()
      .filter(|call| call.stage == stage)
      .map(|call| call.live_or_replayed)
      .collect()
  };
  assert!(
    base
      .calls
      .iter()
      .all(|call| call.live_or_replayed == LiveOrReplayed::Live)
  );
  assert_eq!(
    origins(enhanced, Stage::Review),
    [LiveOrReplayed::Replayed],
    "the enhanced arm must not bill the baseline's review request again"
  );
  assert_eq!(origins(enhanced, Stage::Judge), [LiveOrReplayed::Live]);
  assert!(
    (base.usage.cost_usd - 0.01).abs() < 1e-9,
    "{:?}",
    base.usage
  );
  assert!(
    (enhanced.usage.cost_usd - 0.002).abs() < 1e-9,
    "{:?}",
    enhanced.usage
  );
  assert_eq!(
    base.tally,
    Tally {
      caught: 1,
      missed: 0,
      flagged: 0,
      clear: 1,
      unclassified: 0
    }
  );
  assert_eq!(
    base.usage,
    UsageTotals {
      tokens_in: 100,
      tokens_out: 40,
      cost_usd: base.usage.cost_usd
    }
  );
}

#[test]
fn a_live_run_bills_and_records_each_distinct_request_once() {
  let fixtures = golden().expect("fixture");
  let origin = Fixture::start().expect("loopback");
  origin
    .route(REVIEW, Reply::new(200, answer()))
    .expect("route");
  origin
    .route(JEV, Reply::new(200, judgment()))
    .expect("route");
  let store = tempfile::tempdir().expect("fs");
  let live = record(&fixtures, &origin, store.path()).expect("live run");
  assert_eq!(live.unverified.len(), 0, "{:?}", live.unverified);
  let [base, enhanced] = live.report.arms.as_slice() else {
    panic!("two arms: {}", live.report.arms.len());
  };
  assert!(base.calls.iter().all(|call| call.stage == Stage::Review));
  assert!(enhanced.calls.iter().any(|call| call.stage == Stage::Judge));
  assert_billed_live(base, enhanced);
  // Both arms send the one review request; only the enhanced arm adds a judgment.
  let names = file_names(store.path()).expect("fs");
  assert_eq!(names.len(), 2, "{names:?}");
  assert!(names.iter().any(|name| name.starts_with("judge-")));
  assert!(names.iter().any(|name| name.starts_with("review-")));
  // One review request (baseline) and one judgment (enhanced): no duplicate billing.
  let sent = origin.requests().expect("requests");
  assert_eq!(sent.len(), 2, "{} requests", sent.len());
}

#[test]
fn a_replayed_call_costs_nothing_and_matches_what_the_live_call_found() {
  let fixtures = golden().expect("fixture");
  let origin = Fixture::start().expect("loopback");
  origin
    .route(REVIEW, Reply::new(200, answer()))
    .expect("route");
  origin
    .route(JEV, Reply::new(200, judgment()))
    .expect("route");
  let store = tempfile::tempdir().expect("fs");
  let live = record(&fixtures, &origin, store.path()).expect("live run");
  let sent = origin.requests().expect("requests").len();

  // A fresh materialization of the case still hashes to the recorded requests.
  let replayed = replay(&fixtures, store.path()).expect("replay run");
  assert_eq!(replayed.unverified.len(), 0, "{:?}", replayed.unverified);
  assert_eq!(replayed.report.arms.len(), 2);
  for (arm, was) in replayed.report.arms.iter().zip(&live.report.arms) {
    assert!(
      arm.calls.iter().all(|call| {
        call.live_or_replayed == LiveOrReplayed::Replayed && call.cost == Some(0.0)
      })
    );
    assert_eq!(arm.calls.len(), was.calls.len());
    assert_eq!(arm.usage.cost_usd.to_bits(), 0.0_f64.to_bits());
    assert_eq!(
      (arm.usage.tokens_in, arm.usage.tokens_out),
      (was.usage.tokens_in, was.usage.tokens_out)
    );
    assert_eq!(arm.matches, was.matches);
    assert_eq!(arm.findings, was.findings);
  }
  assert_eq!(
    origin.requests().expect("requests").len(),
    sent,
    "replay sent nothing"
  );
}
