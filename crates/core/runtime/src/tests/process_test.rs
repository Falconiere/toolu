use std::time::Duration;

use super::{RunError, Spec, group, run};
use crate::env::Env;

fn sh(script: &str) -> Spec {
  Spec::new(["sh", "-c", script])
}

#[test]
fn a_command_reports_its_streams_and_exit_code() {
  let output = run(&sh("echo out; echo err >&2; exit 3")).unwrap();
  assert_eq!(output.stdout, "out\n");
  assert_eq!(output.stderr, "err\n");
  assert_eq!(output.exit_code, 3);
  assert!(!output.timed_out && !output.truncated);
}

#[test]
fn stdin_reaches_the_child_and_cwd_and_env_are_exact() {
  let dir = tempfile::tempdir().unwrap();
  let mut spec = sh("cat; echo \" $A-${B:-unset} $(pwd)\"");
  spec.stdin = b"hello".to_vec();
  spec.cwd = Some(dir.path().to_path_buf());
  let path = std::env::var("PATH").unwrap();
  spec.env = Some(Env::from_pairs([("PATH", path.as_str()), ("A", "1")]));
  let output = run(&spec).unwrap();
  let cwd = std::fs::canonicalize(dir.path()).unwrap();
  assert_eq!(output.stdout, format!("hello 1-unset {}\n", cwd.display()));
}

#[test]
fn the_deadline_terminates_the_whole_group() {
  let mut spec = sh("sleep 30 & sleep 30");
  spec.timeout = Duration::from_millis(200);
  let output = run(&spec).unwrap();
  assert!(output.timed_out);
  assert!(
    output.duration < Duration::from_secs(3),
    "{:?}",
    output.duration
  );
  assert!(!group::alive(output.pid));
}

#[test]
fn a_child_that_ignores_sigterm_is_killed() {
  let mut spec = sh("trap '' TERM; sleep 30");
  spec.timeout = Duration::from_millis(100);
  let output = run(&spec).unwrap();
  assert!(output.timed_out);
  assert!(!group::alive(output.pid));
}

#[test]
fn output_past_the_budget_is_dropped() {
  let mut spec = Spec::new(["head", "-c", "4096", "/dev/zero"]);
  spec.max_output_bytes = 100;
  let output = run(&spec).unwrap();
  assert!(output.truncated);
  assert_eq!(output.stdout.len(), 100);
  assert_eq!(output.exit_code, 0);
}

#[test]
fn a_child_that_closes_stdin_early_is_not_an_error() {
  let mut spec = Spec::new(["true"]);
  spec.stdin = vec![b'x'; 4 * 1024 * 1024];
  assert_eq!(run(&spec).unwrap().exit_code, 0);
}

#[test]
fn a_signalled_child_exits_with_128_plus_the_signal() {
  assert_eq!(run(&sh("kill -9 $$")).unwrap().exit_code, 137);
}

#[test]
fn a_descendant_outliving_the_child_is_waited_for() {
  let output = run(&sh("(sleep 0.3; echo late) & echo early")).unwrap();
  assert_eq!(output.stdout, "early\nlate\n");
  assert!(!output.timed_out);
}

#[test]
fn unusable_specs_fail_before_spawning() {
  assert_eq!(
    run(&Spec::new(Vec::<String>::new())),
    Err(RunError::EmptyArgv)
  );
  assert_eq!(run(&Spec::new([""])), Err(RunError::EmptyArgv));
  let mut zero = Spec::new(["true"]);
  zero.timeout = Duration::ZERO;
  assert_eq!(run(&zero), Err(RunError::ZeroTimeout));
  let mut endless = Spec::new(["true"]);
  endless.timeout = Duration::MAX;
  assert_eq!(run(&endless), Err(RunError::TimeoutTooLong));
  let missing = run(&Spec::new(["/nonexistent/toolu-no-such-program"]));
  assert!(
    matches!(missing, Err(RunError::Spawn(message)) if message.contains("toolu-no-such-program"))
  );
}

#[test]
fn a_process_that_left_the_group_holding_stdin_does_not_block_the_run() {
  let script =
    "exec 3<&0; perl -MPOSIX -e 'POSIX::setsid() or die; sleep 2' <&3 >/dev/null 2>&1 & exit 0";
  let mut spec = sh(script);
  spec.stdin = vec![b'x'; 1024 * 1024];
  spec.timeout = Duration::from_millis(300);
  let started = std::time::Instant::now();
  let output = run(&spec).unwrap();
  assert_eq!(output.exit_code, 0);
  assert!(
    started.elapsed() < Duration::from_millis(1500),
    "{:?}",
    started.elapsed()
  );
}
