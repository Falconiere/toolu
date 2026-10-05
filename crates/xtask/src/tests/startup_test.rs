use std::path::{Path, PathBuf};
use std::time::Duration;

use super::{BUDGET, judge, load, run};
use crate::Verdict;
use crate::options::Options;
use crate::tests::install;

/// This repository, whose real budget file the checks read.
fn repo() -> PathBuf {
  std::fs::canonicalize(concat!(env!("CARGO_MANIFEST_DIR"), "/../..")).unwrap()
}

fn stand_in(dir: &Path, body: &str) -> PathBuf {
  let bin = dir.join("toolu");
  install(&bin, &format!("#!/bin/sh\n{body}\n"));
  bin
}

/// A repository root whose own budget file is `budget` (a JSON object body).
fn root_with(budget: &str) -> tempfile::TempDir {
  let dir = tempfile::tempdir().unwrap();
  std::fs::create_dir_all(dir.path().join("benchmarks")).unwrap();
  std::fs::write(dir.path().join(BUDGET), budget).unwrap();
  dir
}

/// A root budgeting `--version` at `wall_ms` for the nearest-rank `percentile` of `runs`.
fn root_budgeting(
  wall_ms: u64,
  warmup: usize,
  runs: usize,
  percentile: usize,
) -> tempfile::TempDir {
  root_with(&format!(
    r#"{{"version":1,"command":["--version"],"warmup":{warmup},"runs":{runs},"percentile":{percentile},"wallMs":{wall_ms}}}"#
  ))
}

fn options(root: PathBuf, bin: &Path) -> Options {
  Options {
    root,
    bin: Some(bin.to_path_buf()),
    ..Options::default()
  }
}

#[test]
fn the_budget_is_the_documented_one() {
  let budget = load(&repo()).unwrap();
  assert_eq!(budget.command, ["--version"]);
  assert_eq!((budget.percentile, budget.wall_ms), (50, 4));
  assert_eq!((budget.warmup, budget.runs), (3, 30));
}

#[test]
fn a_fast_binary_passes() {
  let dir = tempfile::tempdir().unwrap();
  let bin = stand_in(dir.path(), "echo 'toolu 0.0.0'");
  let root = root_budgeting(1000, 1, 3, 50);
  assert_eq!(
    run(&options(root.path().to_path_buf(), &bin)),
    Ok(Verdict::Clean)
  );
}

#[test]
fn a_slow_binary_fails_naming_the_budget() {
  let dir = tempfile::tempdir().unwrap();
  // `sleep` is a lower bound: every run takes at least 50 ms against a 5 ms budget.
  let bin = stand_in(dir.path(), "sleep 0.05; echo 'toolu 0.0.0'");
  let root = root_budgeting(5, 1, 3, 50);
  assert_eq!(
    run(&options(root.path().to_path_buf(), &bin)),
    Ok(Verdict::Findings)
  );
  let budget = load(&repo()).unwrap();
  let report = judge(&[Duration::from_millis(10); 30], &budget).unwrap_err();
  assert!(
    report.ends_with("over 30 runs (budget 4 ms): over budget"),
    "{report}"
  );
  assert!(report.contains("wall p50 10.00 ms"), "{report}");
}

#[test]
fn the_judged_percentile_is_the_nearest_rank() {
  let budget = load(&repo()).unwrap();
  let mut walls: Vec<Duration> = (1..=30).map(Duration::from_millis).collect();
  walls.reverse();
  let report = judge(&walls, &budget).unwrap_err();
  assert!(
    report.contains("wall p50 15.00 ms, p90 27.00 ms"),
    "{report}"
  );
  let fast = judge(&[Duration::from_millis(4); 5], &budget).unwrap();
  assert!(fast.contains("p50 4.00 ms"), "{fast}");
}

#[test]
fn a_binary_that_is_not_toolu_or_fails_is_a_setup_error() {
  let dir = tempfile::tempdir().unwrap();
  let other = stand_in(dir.path(), "echo 'git version 2'");
  let err = run(&options(repo(), &other)).unwrap_err();
  assert!(err.contains("not `toolu <version>`"), "{err}");
  let failing = stand_in(dir.path(), "exit 3");
  assert!(
    run(&options(repo(), &failing))
      .unwrap_err()
      .contains("exited")
  );
  let missing = run(&Options {
    root: repo(),
    ..Options::default()
  });
  assert_eq!(missing, Err("check-startup needs --bin FILE".to_owned()));
}

#[test]
fn a_binary_that_does_not_exist_cannot_be_run() {
  let dir = tempfile::tempdir().unwrap();
  let err = run(&options(repo(), &dir.path().join("absent"))).unwrap_err();
  assert!(err.starts_with("cannot run "), "{err}");
  assert!(err.contains("absent --version"), "{err}");
}

#[test]
fn warm_up_spawns_run_but_are_not_measured() {
  let dir = tempfile::tempdir().unwrap();
  let calls = dir.path().join("calls");
  // The first two spawns (the warm-up) take 300 ms; the budget judges the slowest measured run.
  let bin = stand_in(
    dir.path(),
    &format!(
      "echo x >> '{0}'\nif [ $(wc -l < '{0}') -le 2 ]; then sleep 0.3; fi\necho 'toolu 0.0.0'",
      calls.display()
    ),
  );
  let root = root_budgeting(250, 2, 3, 100);
  assert_eq!(
    run(&options(root.path().to_path_buf(), &bin)),
    Ok(Verdict::Clean)
  );
  assert_eq!(std::fs::read_to_string(&calls).unwrap().lines().count(), 5);
}

#[test]
fn a_malformed_budget_is_a_setup_error() {
  let dir = tempfile::tempdir().unwrap();
  std::fs::create_dir_all(dir.path().join("benchmarks")).unwrap();
  std::fs::write(dir.path().join(BUDGET), r#"{"version":1,"wallMs":4}"#).unwrap();
  assert!(
    load(dir.path())
      .unwrap_err()
      .starts_with(&format!("{BUDGET} is malformed"))
  );
  std::fs::write(
    dir.path().join(BUDGET),
    r#"{"version":2,"command":["--version"],"warmup":0,"runs":1,"percentile":50,"wallMs":4}"#,
  )
  .unwrap();
  assert!(load(dir.path()).unwrap_err().contains("needs version 1"));
  assert!(
    load(&dir.path().join("absent"))
      .unwrap_err()
      .starts_with("cannot read")
  );
}

#[test]
fn a_budget_that_measures_nothing_or_judges_no_percentile_is_rejected() {
  let needs =
    format!("{BUDGET} needs version 1, a command, at least one run and a percentile of 1 to 100");
  let rejected = [
    r#"{"version":1,"command":["--version"],"warmup":0,"runs":0,"percentile":50,"wallMs":4}"#,
    r#"{"version":1,"command":[],"warmup":0,"runs":1,"percentile":50,"wallMs":4}"#,
    r#"{"version":1,"command":["--version"],"warmup":0,"runs":1,"percentile":0,"wallMs":4}"#,
    r#"{"version":1,"command":["--version"],"warmup":0,"runs":1,"percentile":101,"wallMs":4}"#,
  ];
  for budget in rejected {
    let root = root_with(budget);
    assert_eq!(load(root.path()).unwrap_err(), needs, "{budget}");
  }
  for percentile in [1, 100] {
    let root = root_budgeting(4, 0, 1, percentile);
    assert_eq!(load(root.path()).unwrap().percentile, percentile);
  }
}
