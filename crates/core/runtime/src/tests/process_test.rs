use std::time::Duration;

use super::{RunError, Spec, Wait, group, run};
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

#[test]
fn stdout_bytes_keep_what_the_lossy_text_replaces() {
  let output = run(&sh("printf 'a\\377\\376b'")).unwrap();
  assert_eq!(output.stdout_bytes, b"a\xff\xfeb".to_vec());
  assert_eq!(output.stdout, "a\u{fffd}\u{fffd}b");
}

#[test]
fn streams_mode_returns_once_the_child_exited_and_its_streams_closed() {
  let mut spec = sh("sleep 30 >/dev/null 2>&1 & echo started");
  spec.wait = Wait::Streams;
  spec.timeout = Duration::from_secs(10);
  let output = run(&spec).unwrap();
  assert_eq!(output.stdout, "started\n");
  assert!(!output.timed_out);
  assert!(
    output.duration < Duration::from_secs(5),
    "{:?}",
    output.duration
  );
  group::terminate(output.pid).unwrap();
}

#[test]
fn streams_mode_still_waits_for_a_descendant_holding_stdout_and_kills_it_at_the_deadline() {
  let mut spec = sh("sleep 30 & echo started");
  spec.wait = Wait::Streams;
  spec.timeout = Duration::from_millis(300);
  let output = run(&spec).unwrap();
  assert!(output.timed_out);
  assert_eq!(output.stdout, "started\n");
  assert!(!group::alive(output.pid));
}

#[test]
fn group_mode_is_the_default() {
  assert_eq!(Spec::new(["true"]).wait, Wait::Group);
}

#[test]
fn run_with_calls_back_with_the_group_before_stdin_is_written() {
  let dir = tempfile::tempdir().unwrap();
  let marker = dir.path().join("marker");
  let mut spec = sh(&format!(
    "read -r go; echo \"$go $(cat {})\"",
    marker.display()
  ));
  spec.stdin = b"go\n".to_vec();
  let mut seen = 0;
  let output = super::run_with(&spec, &mut |pid| {
    seen = pid;
    std::fs::write(&marker, "after-callback").map_err(|err| err.to_string())
  })
  .unwrap();
  assert_eq!(seen, output.pid);
  assert_eq!(output.stdout, "go after-callback\n");
}

#[test]
fn a_failed_callback_stops_the_group_and_is_returned() {
  let spec = sh("sleep 30");
  let mut group_id = 0;
  let result = super::run_with(&spec, &mut |pid| {
    group_id = pid;
    Err("lease lost".to_owned())
  });
  assert_eq!(result, Err(RunError::Callback("lease lost".to_owned())));
  assert!(!group::alive(group_id));
}
