//! One step's run against a real repository and check.

use toolu_runtime::json::ordered::Ordered;
use toolu_state::telemetry::TelemetryEvent;

use super::{Target, prior_of, run_step, step_event};
use crate::ledger::context::test_repo::Repo;
use crate::ledger::context::{RunFlags, prepare};
use crate::ledger::entries::RunOutcome;
use crate::ledger::io::Output;
use crate::ledger::jq::{parse_json, string};

#[test]
fn a_step_runs_its_checks_stamps_its_scope_and_leaves_no_stage() {
  let repo = Repo::new().unwrap();
  let plan = "## Steps (machine-readable)\n```json\n[{\"id\":\"s1\",\"title\":\"t\",\"check\":\"echo one\"},{\"id\":\"s1\",\"title\":\"u\",\"check\":\"echo two; exit 3\"}]\n```\n";
  std::fs::write(repo.root.join("plan.md"), plan).unwrap();
  let opts = repo.opts();
  let flags = RunFlags::default();
  let ctx = prepare("plan.md", &flags, &opts).unwrap();
  let matches: Vec<&Ordered> = ctx.steps.iter().collect();
  let target = Target {
    id: "s1",
    matches: &matches,
    at: "[1/2]",
    scope_now: "SCOPE",
  };
  let mut out = Output::default();
  let entries = run_step(&ctx, &mut out, &target).unwrap();
  assert_eq!(entries.len(), 2);
  let first = entries.first().unwrap();
  assert_eq!(first.get("status"), Some(&string("red")));
  assert_eq!(first.get("evidence_tail"), Some(&string("one\ntwo")));
  assert_eq!(first.get("scope_sha"), Some(&string("SCOPE")));
  let result = out.result(1, None);
  assert!(
    result
      .stderr
      .starts_with("plan-ledger: [1/2] s1: running check\nplan-ledger: [1/2] s1: red (")
  );
  let dir = ctx.ledger_file.parent().unwrap();
  let staged = std::fs::read_dir(dir).unwrap().count();
  assert_eq!(staged, 0, "the output stage is removed");
  assert_eq!(prior_of(&ctx, "s1"), &Ordered::Null);
  repo.sh("true").unwrap();
}

#[test]
fn the_step_event_carries_the_attempt() {
  let run = RunOutcome {
    status: "green",
    exit_code: 0,
    sha: "S".to_owned(),
    evidence: String::new(),
    now: "N".to_owned(),
  };
  let entry = parse_json(r#"{"retries":[{},{}]}"#).unwrap();
  let event = step_event("s9", &run, 4, &entry);
  let expected = TelemetryEvent::StepRun {
    step_id: "s9".to_owned(),
    status: "green".to_owned(),
    exit_code: 0.0,
    duration_s: 4.0,
    attempt: 3.0,
  };
  assert_eq!(event, expected);
}
