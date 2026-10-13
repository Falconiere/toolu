use std::time::Duration;

use super::{first_line, run};
use crate::support::{Sandbox, names};

#[test]
fn a_command_reports_its_status_and_streams() {
  let sb = Sandbox::new();
  sb.stub("talk", "echo out; echo err >&2; exit 3");
  let ran = run(&names(&["talk"]), &sb.env(), Duration::from_secs(10));
  assert_eq!((ran.code, ran.stdout.as_str(), ran.stderr.as_str(), ran.ok()), (3, "out\n", "err\n", false));
  let fine = run(&names(&["talk"]), &sb.env(), Duration::from_secs(10));
  assert!(!fine.ok());
}

#[test]
fn a_missing_program_is_127_and_a_slow_one_times_out() {
  let sb = Sandbox::new();
  let missing = run(&names(&["no-such-host"]), &sb.env(), Duration::from_secs(5));
  assert_eq!(missing.code, 127);
  assert_ne!(missing.stderr, "");
  let empty = run(&[], &sb.env(), Duration::from_secs(5));
  assert_eq!((empty.code, empty.stderr.as_str()), (127, "no command to run"));
  sb.stub("slow", "sleep 5");
  let slow = run(&names(&["slow"]), &sb.env(), Duration::from_secs(1));
  assert_eq!((slow.code, slow.stderr.as_str()), (124, "timed out after 1 s"));
  let zero = run(&names(&["slow"]), &sb.env(), Duration::ZERO);
  assert_eq!((zero.code, zero.stderr.as_str()), (127, "unusable deadline"));
}

#[test]
fn the_first_non_blank_line_is_trimmed() {
  assert_eq!(first_line("\n  \n  boom: no network  \nmore", "x"), "boom: no network");
  assert_eq!(first_line(" \n", "install failed"), "install failed");
}
