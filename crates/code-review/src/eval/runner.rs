//! The eval runner: materialize each case into a Git repository, shape it with the
//! production stages, and run the baseline (Jev judge off) and enhanced (judge on)
//! arms through `review::run` with `Options`, then match what each arm reported
//! against the case's expectations. By default every call is answered from the
//! replay store and nothing is sent; a request the store lacks fails that arm
//! closed as unverified. `Mode::Live` calls the real clients and, with a store,
//! records each exchange for later replays.

use std::cell::RefCell;
use std::time::Instant;

use toolu_runtime::env::Env;

use super::arms::{Mode, Plan};
use super::calibration::calibrate;
use super::expectations::Case;
use super::fixtures::{Fixtures, rev_parse};
use super::gate::violations;
use super::matcher::match_findings;
use super::replay::Store;
use super::report::{Arm, ArmReport, CallRecord, EvalReport, LiveOrReplayed, Revisions, Run};
use super::setting::Setting;
use super::suite::{CaseReport, SCHEMA, SuiteReport};
use crate::memory::BranchState;
use crate::pipeline;
use crate::prepass::{self, PrepassResult};
use crate::review::{self, Call, Input, Options, REPLAYED, Record, RunError, Stage};
use crate::rules::{self, Bundle};
use crate::settings::Settings;
use crate::shape::{self, Shaped};

/// The key a replay run hands the clients: they refuse to build without one, and
/// no request is ever sent, so no real key is read or used.
const REPLAY_KEY: &str = "replay-only-no-request-is-sent";

/// One case's shaped inputs, shared by its arms.
struct Prepared {
  root: std::path::PathBuf,
  env: Env,
  shaped: Shaped,
  rules: Bundle,
  prepass: PrepassResult,
  prior: BranchState,
}

fn describe(err: &RunError) -> String {
  match err {
    RunError::NotEnabled => "not enabled".to_owned(),
    RunError::Unconfigured(why) => why.clone(),
    RunError::Outage { message, .. } => message.clone(),
    RunError::Deadline { .. } => "every review call ran out of its deadline".to_owned(),
  }
}

/// Where a call's answer came from: a replay run never sends a request, so even a
/// call the store could not answer is replayed, not live.
fn origin(mode: &Mode) -> LiveOrReplayed {
  match mode {
    Mode::Replay(_) => LiveOrReplayed::Replayed,
    Mode::Live(_) => LiveOrReplayed::Live,
  }
}

/// Whether `call` was answered: a live call says `ok`, a replayed one `replayed`.
fn answered(call: &Call) -> bool {
  call.outcome == "ok" || call.outcome == REPLAYED
}

/// The replay lookup of a run: only a replay run has one, and every request the
/// store lacks or cannot read is noted in `problems`.
fn lookup<'a>(
  plan: &'a Plan<'_>,
  problems: &'a RefCell<Vec<String>>,
) -> impl Fn(Stage, &str) -> Option<String> + 'a {
  move |stage, request| {
    let Mode::Replay(store) = plan.mode else {
      return None;
    };
    match store.read(stage, request) {
      Ok(Some(body)) => Some(body),
      Ok(None) => {
        let key = Store::key(stage, request);
        problems.borrow_mut().push(format!("replay miss: {key}"));
        None
      }
      Err(why) => {
        problems.borrow_mut().push(why);
        None
      }
    }
  }
}

/// The recorder of a live run with a store; a write that fails is noted in `problems`.
fn recorder<'a>(
  plan: &'a Plan<'_>,
  problems: &'a RefCell<Vec<String>>,
) -> impl Fn(&Record<'_>) + 'a {
  move |exchange| {
    if let Mode::Live(Some(store)) = plan.mode
      && !exchange.response.is_empty()
      && let Err(why) = store.write(exchange.stage, exchange.request, exchange.response)
    {
      problems.borrow_mut().push(why);
    }
  }
}

/// What the clients see: a replay run gets a placeholder key, never the real one.
fn client_env(prepared: &Prepared, plan: &Plan<'_>) -> Env {
  match plan.mode {
    Mode::Replay(_) => prepared.env.clone().with("OPENROUTER_API_KEY", REPLAY_KEY),
    Mode::Live(_) => prepared.env.clone(),
  }
}

/// Run one arm at one setting; `Err` names why it could not be verified.
fn run_arm(
  prepared: &Prepared,
  plan: &Plan<'_>,
  (setting, arm): (Setting, Arm),
  case: &Case,
) -> Result<ArmReport, String> {
  let problems = RefCell::new(Vec::<String>::new());
  let (replay, record) = (lookup(plan, &problems), recorder(plan, &problems));
  let mut options = Options {
    openrouter: plan.openrouter.clone(),
    jev: plan.jev.clone(),
    skip_judge: arm == Arm::Baseline,
    ..Options::default()
  };
  match plan.mode {
    Mode::Replay(_) => options.replay = Some(&replay),
    Mode::Live(_) => options.recorder = Some(&record),
  }
  let input = Input {
    root: &prepared.root,
    shaped: &prepared.shaped,
    rules: &prepared.rules,
    prepass: &prepared.prepass,
    memory: &prepared.prior.memory,
    settings: &Settings {
      enabled: true,
      ..Settings::default()
    },
    flag: Some(setting.flag()),
  };
  let started = Instant::now();
  let result = review::run(&client_env(prepared, plan), &input, &options);
  let runtime_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
  let problems = problems.take();
  if !problems.is_empty() {
    return Err(problems.join("; "));
  }
  let reviewed = result.map_err(|err| describe(&err))?;
  arm_report(prepared, plan, case, (setting, arm, runtime_ms), reviewed)
}

/// Ground `reviewed`, match it against `case` and shape the arm.
///
/// # Errors
/// A call failed or the review left a path unreviewed: the arm is unverified, since
/// an outage or an incomplete review must not read as a pass or an ordinary miss.
fn arm_report(
  prepared: &Prepared,
  plan: &Plan<'_>,
  case: &Case,
  (setting, arm, runtime_ms): (Setting, Arm, u64),
  reviewed: review::Reviewed,
) -> Result<ArmReport, String> {
  let failed: Vec<String> = reviewed
    .calls
    .iter()
    .filter(|call| !answered(call))
    .map(|call| format!("{} call: {}", call.stage.name(), call.outcome))
    .collect();
  let calls: Vec<CallRecord> = reviewed
    .calls
    .iter()
    .map(|call| CallRecord::from_call(call, origin(plan.mode)))
    .collect();
  let effort = reviewed.effort.level;
  let (report, _recap) = pipeline::completed(
    &prepared.shaped,
    &prepared.prepass,
    &prepared.prior,
    reviewed,
  );
  if !report.is_complete() {
    let mut why = failed;
    why.extend(
      report
        .error
        .iter()
        .map(|error| format!("review error: {}", error.message)),
    );
    why.extend(
      report
        .coverage
        .unreviewed
        .iter()
        .map(|item| format!("unreviewed {}: {}", item.path, item.reason)),
    );
    return Err(format!("incomplete review: {}", why.join(", ")));
  }
  if !failed.is_empty() {
    return Err(format!("failed calls: {}", failed.join(", ")));
  }
  let run = Run {
    arm,
    setting,
    effort,
    coverage: report.coverage.clone(),
    calls,
    runtime_ms,
  };
  let outcome = match_findings(&case.expectations, &report.findings);
  Ok(ArmReport::new(run, &report.findings, outcome))
}

fn prepare(
  fixtures: &Fixtures,
  case: &Case,
  plan: &Plan<'_>,
) -> Result<(tempfile::TempDir, Prepared), String> {
  let repo = fixtures.materialize_repo(case)?;
  let env = plan
    .env
    .clone()
    .with("GIT_CONFIG_NOSYSTEM", "1")
    .with("GIT_CONFIG_GLOBAL", "/dev/null")
    .with("XDG_CONFIG_HOME", "/dev/null");
  let root = repo.path().to_path_buf();
  let fail = |stage: &str, err: &dyn std::fmt::Debug| format!("{}: {stage}: {err:?}", case.id);
  let shaped =
    shape::shape(&env, &root, Some(fixtures.base_rev(case))).map_err(|err| fail("shape", &err))?;
  let rules = rules::gather(&env, &root, &shaped.merge_base, &shaped.gate_files)
    .map_err(|err| fail("rules", &err))?;
  let prepass = prepass::run(&env, &root, &shaped);
  let prior = BranchState {
    memory: fixtures.memory(case)?.unwrap_or_default(),
    ..BranchState::default()
  };
  let prepared = Prepared {
    root,
    env,
    shaped,
    rules,
    prepass,
    prior,
  };
  Ok((repo, prepared))
}

/// Run every selected arm of `case`.
///
/// # Errors
/// The case cannot be materialized or shaped; an arm that cannot be verified is
/// reported in `unverified`, not here.
pub fn run_case(fixtures: &Fixtures, case: &Case, plan: &Plan<'_>) -> Result<CaseReport, String> {
  let (repo, prepared) = prepare(fixtures, case, plan)?;
  let mut arms = Vec::new();
  let mut unverified = Vec::new();
  if plan
    .settings
    .iter()
    .all(|setting| plan.arms.arms(*setting).is_empty())
  {
    unverified.push(format!(
      "no arm selected: {:?} runs nothing at the chosen effort",
      plan.arms
    ));
  }
  for setting in plan.settings {
    for arm in plan.arms.arms(*setting) {
      match run_arm(&prepared, plan, (*setting, arm), case) {
        Ok(report) => arms.push(report),
        Err(why) => unverified.push(format!("{arm:?} at {}: {why}", setting.name())),
      }
    }
  }
  let revisions = Revisions {
    base: prepared.shaped.merge_base,
    tip: rev_parse(repo.path(), "HEAD")?,
  };
  Ok(CaseReport {
    id: case.id.clone(),
    class: case.class,
    report: EvalReport::new(revisions, arms),
    unverified,
    gate_violations: case.gate.as_ref().map(violations).unwrap_or_default(),
  })
}

/// Run every case of `fixtures`; calibration is added when several settings ran.
///
/// # Errors
/// As [`run_case`], for the first case that fails.
pub fn run_all(fixtures: &Fixtures, plan: &Plan<'_>) -> Result<SuiteReport, String> {
  let cases = fixtures
    .set()
    .cases
    .iter()
    .map(|case| run_case(fixtures, case, plan))
    .collect::<Result<Vec<_>, _>>()?;
  let inputs: Vec<(&str, &[ArmReport])> = cases
    .iter()
    .map(|case| (case.id.as_str(), case.report.arms.as_slice()))
    .collect();
  let calibration = (plan.settings.len() > 1).then(|| calibrate(&inputs));
  Ok(SuiteReport {
    schema: SCHEMA,
    cases,
    calibration,
  })
}

#[cfg(test)]
#[path = "tests/runner_test.rs"]
mod tests;
