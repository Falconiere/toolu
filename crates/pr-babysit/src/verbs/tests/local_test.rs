use std::path::PathBuf;

use serde_json::{Value, json};
use tempfile::TempDir;
use toolu_protocol::exit::Exit;
use toolu_runtime::env::Env;

use crate::fixture;
use crate::reduce::{Paths, reduce};
use crate::verbs::tests::{initial_state, read, run, world};

const NOW: &str = "2026-09-19T12:00:00Z";

fn state(dir: &TempDir) -> PathBuf {
  initial_state(dir.path())
}

fn message(error: &Value) -> &str {
  error["errors"][0]["message"].as_str().unwrap_or_default()
}

#[test]
fn reduce_state_prints_and_writes_what_the_reducer_returns() {
  let dir = TempDir::new().unwrap();
  let (state_out, result_out) = (
    dir.path().join("state.json"),
    dir.path().join("result.json"),
  );
  let snapshot = fixture::path("snapshots/toolu-165.json");
  let env = Env::default();
  let args = [
    "reduce-state",
    "--snapshot",
    snapshot.to_str().unwrap(),
    "--now",
    NOW,
    "--state-path",
    "/state.json",
    "--snapshot-path",
    "/snapshot.json",
    "--state-out",
    state_out.to_str().unwrap(),
    "--result-out",
    result_out.to_str().unwrap(),
  ];
  let (exit, printed) = run(&args, &world(&env, None));
  assert_eq!(exit, Exit::Success);
  let paths = Paths {
    state: "/state.json",
    snapshot: "/snapshot.json",
    result: "/state.result.json",
  };
  let reduced = reduce(
    &fixture::json("snapshots/toolu-165.json"),
    None,
    NOW,
    &paths,
  );
  assert_eq!(
    printed,
    json!({"state": reduced.state, "result": reduced.result})
  );
  assert_eq!(read(&state_out), reduced.state);
  assert_eq!(read(&result_out), reduced.result);
}

#[test]
fn reduce_state_continues_from_the_previous_state_of_the_same_slot() {
  let dir = TempDir::new().unwrap();
  let snapshot = fixture::path("snapshots/toolu-165.json");
  let env = Env::default();
  let world = world(&env, None);
  let first = [
    "reduce-state",
    "--snapshot",
    snapshot.to_str().unwrap(),
    "--now",
    NOW,
  ];
  let (_, printed) = run(&first, &world);
  let state = dir.path().join("prev.json");
  std::fs::write(&state, printed["state"].to_string()).unwrap();
  let later = [
    "--state",
    state.to_str().unwrap(),
    "--now",
    "2026-09-19T12:03:00Z",
  ];
  let (exit, next) = run(&[&first[..3], &later].concat(), &world);
  assert_eq!(exit, Exit::Success);
  assert_eq!(next["state"]["totalTicks"], 2);
}

#[test]
fn reduce_state_refuses_bad_input_with_its_own_code() {
  let dir = TempDir::new().unwrap();
  let env = Env::default();
  let world = world(&env, None);
  let snapshot = fixture::path("snapshots/toolu-115.json");
  let base = ["reduce-state", "--snapshot", snapshot.to_str().unwrap()];
  let (exit, error) = run(&[&base[..], &["--now", "noon"]].concat(), &world);
  assert_eq!(exit, Exit::Usage);
  assert_eq!(
    message(&error),
    "--now must be an ISO-8601 UTC timestamp (YYYY-MM-DDTHH:MM:SSZ)"
  );
  let other = state(&dir);
  let mismatched = [
    &base[..],
    &["--now", NOW, "--state", other.to_str().unwrap()],
  ]
  .concat();
  let (exit, error) = run(&mismatched, &world);
  assert_eq!(exit, Exit::Failure);
  assert_eq!(error["errors"][0]["code"], "slot_mismatch");
  let missing = dir.path().join("none.json");
  let gone = [
    "reduce-state",
    "--snapshot",
    missing.to_str().unwrap(),
    "--now",
    NOW,
  ];
  let (_, error) = run(&gone, &world);
  assert_eq!(error["errors"][0]["code"], "invalid_json");
  assert_eq!(
    message(&error),
    format!("snapshot not found: {}", missing.display())
  );
}

#[test]
fn record_writes_rounds_flags_and_the_terminal_status() {
  let dir = TempDir::new().unwrap();
  let path = state(&dir);
  let file = path.to_str().unwrap();
  let env = Env::default();
  let world = world(&env, None);
  let round = [
    "record",
    "round",
    "--state-file",
    file,
    "--had-rejection",
    "true",
    "--fix-pushed",
  ];
  assert_eq!(run(&round, &world).0, Exit::Success);
  let flag = [
    "record",
    "flag-injection",
    "--state-file",
    file,
    "--thread",
    "PRRT_a",
  ];
  assert_eq!(run(&flag, &world).0, Exit::Success);
  let status = [
    "record",
    "status",
    "--state-file",
    file,
    "--status",
    "complete",
  ];
  assert_eq!(run(&status, &world).0, Exit::Success);
  let recorded = read(&path);
  assert_eq!(recorded["pr"]["lastRoundHadRejection"], true);
  assert!(recorded["actions"]["flagged"].get("PRRT_a").is_some());
  assert_eq!(recorded["status"], "complete");
}

#[test]
fn route_fix_plans_inline_from_the_project_config() {
  let dir = TempDir::new().unwrap();
  let project = dir.path().join("project");
  std::fs::create_dir_all(project.join(".claude")).unwrap();
  let config = json!({"prBabysit": {"dispatch": "inline", "jev": false}});
  std::fs::write(
    project.join(".claude/toolu.config.json"),
    config.to_string(),
  )
  .unwrap();
  let home = dir.path().to_str().unwrap();
  let env = Env::from_pairs([
    ("HOME", home),
    ("TOOLU_PROJECT_DIR", project.to_str().unwrap()),
  ]);
  let items = fixture::path("items/review-items.json");
  let args = [
    "route-fix",
    "--items",
    items.to_str().unwrap(),
    "--now",
    "2026-09-28T12:00:00Z",
    "--no-jev",
  ];
  for args in [&args[..], &args[..5]] {
    let (exit, plan) = run(args, &world(&env, None));
    assert_eq!(exit, Exit::Success, "{plan}");
    assert_eq!(
      plan,
      fixture::json("route/route-plans.json")["heuristicInline"],
      "`prBabysit.jev: false` is --no-jev"
    );
  }
}

#[test]
fn route_fix_refuses_a_controller_that_cannot_fix() {
  let env = Env::default();
  let mut world = world(&env, None);
  world.host = toolu_protocol::host::Host::Hermes;
  let (exit, _) = run(&["route-fix", "--items", "i.json", "--no-jev"], &world);
  assert_ne!(exit, Exit::Success);
}

#[test]
fn fixer_report_writes_the_groups_report() {
  let dir = TempDir::new().unwrap();
  let report = dir.path().join("r1g1.report.json");
  let env = Env::default();
  let args = [
    "fixer-report",
    report.to_str().unwrap(),
    "done",
    "--note",
    "fixed the test",
  ];
  let (exit, written) = run(&args, &world(&env, None));
  assert_eq!(exit, Exit::Success);
  assert_eq!(read(&report)["status"], "done");
  assert_eq!(written["note"], "fixed the test");
}

#[test]
fn dispatch_fix_cleanup_dry_run_and_stop_end_an_idle_round() {
  let dir = TempDir::new().unwrap();
  let path = state(&dir);
  let file = path.to_str().unwrap();
  let env = Env::default();
  let world = world(&env, None);
  let dry = ["dispatch-fix", "cleanup", "--state-file", file, "--dry-run"];
  let (exit, planned) = run(&dry, &world);
  assert_eq!(exit, Exit::Success, "{planned}");
  assert_eq!(planned["dryRun"], true);
  let (exit, stopped) = run(&["stop", "--state-file", file], &world);
  assert_eq!(exit, Exit::Success, "{stopped}");
  assert_eq!(stopped["cleanup"]["note"], "no herdr worktree recorded");
  assert_eq!(read(&path)["status"], "cancelled");
}

#[test]
fn dispatch_fix_wait_without_a_round_says_so() {
  let dir = TempDir::new().unwrap();
  let path = state(&dir);
  let env = Env::default();
  let args = [
    "dispatch-fix",
    "wait",
    "--state-file",
    path.to_str().unwrap(),
    "--timeout-seconds",
    "x",
  ];
  let (exit, error) = run(&args, &world(&env, None));
  assert_eq!(exit, Exit::Usage);
  assert_eq!(message(&error), "--timeout-seconds must be a whole number");
}
