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
}

#[test]
fn a_fast_binary_passes() {
  let dir = tempfile::tempdir().unwrap();
  let bin = stand_in(dir.path(), "echo 'toolu 0.0.0'");
  assert_eq!(run(&options(repo(), &bin)), Ok(Verdict::Clean));
}

#[test]
fn a_slow_binary_fails_naming_the_budget() {
  let dir = tempfile::tempdir().unwrap();
  let bin = stand_in(dir.path(), "sleep 0.01; echo 'toolu 0.0.0'");
  assert_eq!(run(&options(repo(), &bin)), Ok(Verdict::Findings));
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
