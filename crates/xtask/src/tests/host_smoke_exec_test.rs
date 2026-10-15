use std::path::Path;
use std::time::Duration;

use super::{exec, run, tail};
use crate::host_smoke_env::ChildEnv;

fn env(base: &Path) -> ChildEnv {
  let path = std::ffi::OsString::from("/usr/bin:/bin");
  ChildEnv::new(Path::new("/x/toolu"), &path, &base.join("shims")).unwrap()
}

fn sh(script: &str) -> Vec<String> {
  ["/bin/sh", "-c", script].map(str::to_owned).to_vec()
}

#[test]
fn run_returns_both_streams_and_writes_the_log() {
  let dir = tempfile::tempdir().unwrap();
  let log = dir.path().join("out.log");
  let out = run(
    &sh("echo out; echo err >&2"),
    dir.path(),
    &env(dir.path()),
    Some(&log),
  )
  .unwrap();
  assert_eq!(out, "out\nerr\n");
  assert_eq!(std::fs::read_to_string(log).unwrap(), "out\nerr\n");
}

#[test]
fn run_fails_on_a_nonzero_exit_with_the_output_tail() {
  let dir = tempfile::tempdir().unwrap();
  let err = run(&sh("echo boom; exit 3"), dir.path(), &env(dir.path()), None).unwrap_err();
  assert!(
    err.contains("exited Some(3)") && err.contains("boom"),
    "{err}"
  );
}

#[test]
fn the_child_sees_the_native_environment_and_no_stdin() {
  let dir = tempfile::tempdir().unwrap();
  let script = "echo \"$TOOLU_BIN|${TOOLU_BUN-unset}\"; cat | wc -c";
  let out = run(&sh(script), dir.path(), &env(dir.path()), None).unwrap();
  let lines: Vec<_> = out.lines().map(str::trim).collect();
  assert_eq!(lines, vec!["/x/toolu|unset", "0"]);
}

#[test]
fn a_child_past_its_deadline_is_killed() {
  let dir = tempfile::tempdir().unwrap();
  let err = exec(
    &sh("sleep 5"),
    dir.path(),
    &env(dir.path()),
    Duration::from_millis(100),
    &mut || {},
  )
  .unwrap_err();
  assert!(err.contains("timed out"), "{err}");
}

#[test]
fn the_tick_runs_while_the_child_does() {
  let dir = tempfile::tempdir().unwrap();
  let mut ticks = 0;
  let done = exec(
    &sh("sleep 0.2"),
    dir.path(),
    &env(dir.path()),
    Duration::from_secs(5),
    &mut || ticks += 1,
  )
  .unwrap();
  assert!(done.success && ticks >= 3, "ticks {ticks}");
}

#[test]
fn an_empty_argv_and_a_missing_program_are_errors() {
  let dir = tempfile::tempdir().unwrap();
  assert!(run(&[], dir.path(), &env(dir.path()), None).is_err());
  let missing = vec!["/no/such/program".to_owned()];
  assert!(
    run(&missing, dir.path(), &env(dir.path()), None)
      .unwrap_err()
      .contains("cannot run")
  );
}

#[test]
fn tail_keeps_the_last_800_characters() {
  let text = format!("{}{}", "a".repeat(100), "b".repeat(800));
  assert_eq!(tail(&text), "b".repeat(800));
  assert_eq!(tail("short"), "short");
}

#[test]
fn a_timeout_kills_the_grandchildren_and_keeps_the_partial_output() {
  let dir = tempfile::tempdir().unwrap();
  let started = std::time::Instant::now();
  let err = exec(
    &sh("echo partial-output; sleep 30 & sleep 30"),
    dir.path(),
    &env(dir.path()),
    Duration::from_secs(2),
    &mut || {},
  )
  .unwrap_err();
  assert!(
    err.contains("timed out") && err.contains("partial-output"),
    "{err}"
  );
  assert!(started.elapsed() < Duration::from_secs(10), "{err}");
}

#[test]
fn a_grandchild_left_behind_does_not_hold_the_pipes_open() {
  let dir = tempfile::tempdir().unwrap();
  let started = std::time::Instant::now();
  let done = exec(
    &sh("echo hi; sleep 30 &"),
    dir.path(),
    &env(dir.path()),
    Duration::from_secs(60),
    &mut || {},
  )
  .unwrap();
  assert_eq!(done.output, "hi\n");
  assert!(started.elapsed() < Duration::from_secs(10));
}
