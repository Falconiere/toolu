use std::path::Path;
use std::time::{Duration, Instant};

use super::{FileOutput, FileSpec, run_to_file};
use crate::env::Env;
use crate::process::{RunError, group};

fn spec(dir: &Path, script: &str, timeout: Option<Duration>) -> FileSpec {
  FileSpec {
    argv: vec!["bash".to_owned(), "-c".to_owned(), script.to_owned()],
    cwd: Some(dir.to_path_buf()),
    env: None,
    out: dir.join("out.log"),
    timeout,
    grace: Duration::from_millis(300),
  }
}

fn read(dir: &Path) -> String {
  std::fs::read_to_string(dir.join("out.log")).unwrap()
}

#[test]
fn stdout_and_stderr_interleave_into_one_file_and_the_exit_code_is_kept() {
  let dir = tempfile::tempdir().unwrap();
  let mut seen = 0;
  let script = "echo one; echo two >&2; echo three; exit 3";
  let output = run_to_file(&spec(dir.path(), script, None), &mut |pid| seen = pid).unwrap();
  assert_eq!(read(dir.path()), "one\ntwo\nthree\n");
  assert_eq!(
    output,
    FileOutput {
      pid: seen,
      exit_code: 3,
      timed_out: false
    }
  );
}

#[test]
fn a_check_that_reads_stdin_sees_end_of_file() {
  let dir = tempfile::tempdir().unwrap();
  let output = run_to_file(&spec(dir.path(), "cat; echo done", None), &mut |_| {}).unwrap();
  assert_eq!((read(dir.path()).as_str(), output.exit_code), ("done\n", 0));
}

#[test]
fn an_overrun_is_stopped_with_its_background_child() {
  let dir = tempfile::tempdir().unwrap();
  let script = "sleep 30 & echo started; wait";
  let started = Instant::now();
  let output = run_to_file(
    &spec(dir.path(), script, Some(Duration::from_millis(300))),
    &mut |_| {},
  )
  .unwrap();
  assert!(output.timed_out);
  assert!(started.elapsed() < Duration::from_secs(5));
  assert_eq!(read(dir.path()), "started\n");
  std::thread::sleep(Duration::from_millis(100));
  assert!(!group::alive(output.pid), "the background sleep was killed");
}

#[test]
fn a_leader_that_ignores_sigterm_is_killed_after_the_grace() {
  let dir = tempfile::tempdir().unwrap();
  let script = "trap '' TERM; echo ready; sleep 30";
  let started = Instant::now();
  let output = run_to_file(
    &spec(dir.path(), script, Some(Duration::from_millis(200))),
    &mut |_| {},
  )
  .unwrap();
  assert!(output.timed_out);
  assert_eq!(output.exit_code, 128 + 9);
  assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn death_by_a_signal_reports_128_plus_its_number() {
  let dir = tempfile::tempdir().unwrap();
  let output = run_to_file(&spec(dir.path(), "kill -USR1 $$", None), &mut |_| {}).unwrap();
  let sigusr1 = nix::sys::signal::Signal::SIGUSR1 as i32;
  assert_eq!(output.exit_code, 128 + sigusr1);
}

#[test]
fn an_unbounded_run_returns_when_the_leader_exits() {
  let dir = tempfile::tempdir().unwrap();
  let script = "(sleep 30 >/dev/null 2>&1 &); echo quick";
  let started = Instant::now();
  let output = run_to_file(&spec(dir.path(), script, None), &mut |_| {}).unwrap();
  assert_eq!(output.exit_code, 0);
  assert!(started.elapsed() < Duration::from_secs(5));
  group::signal(output.pid, nix::sys::signal::Signal::SIGKILL).unwrap();
}

#[test]
fn the_environment_and_directory_are_exact() {
  let dir = tempfile::tempdir().unwrap();
  let mut spec = spec(dir.path(), "echo \"$A $(pwd)\"", None);
  let path = std::env::var("PATH").unwrap();
  spec.env = Some(Env::from_pairs([("PATH", path.as_str()), ("A", "x")]));
  run_to_file(&spec, &mut |_| {}).unwrap();
  let cwd = std::fs::canonicalize(dir.path()).unwrap();
  assert_eq!(read(dir.path()), format!("x {}\n", cwd.display()));
}

#[test]
fn an_unusable_spec_is_an_error() {
  let dir = tempfile::tempdir().unwrap();
  let mut empty = spec(dir.path(), "true", None);
  empty.argv.clear();
  assert_eq!(run_to_file(&empty, &mut |_| {}), Err(RunError::EmptyArgv));
  let mut no_dir = spec(dir.path(), "true", None);
  no_dir.out = dir.path().join("missing/out.log");
  assert!(matches!(
    run_to_file(&no_dir, &mut |_| {}),
    Err(RunError::Spawn(_))
  ));
  let mut no_program = spec(dir.path(), "true", None);
  no_program.argv = vec!["/nonexistent/toolu-test".to_owned()];
  assert!(matches!(
    run_to_file(&no_program, &mut |_| {}),
    Err(RunError::Spawn(_))
  ));
}
