use std::process::ExitCode;

use super::{TASKS, run};

fn words(args: &[&str]) -> Vec<String> {
  args.iter().map(|word| (*word).to_owned()).collect()
}

#[test]
fn every_task_is_listed_in_the_usage() {
  for (name, _) in TASKS {
    assert!(super::USAGE.contains(name), "{name}");
  }
}

#[test]
fn unknown_tasks_bad_options_and_no_task_exit_2() {
  for args in [&[][..], &["nope"][..], &["guardrails", "--x", "y"][..]] {
    assert_eq!(run(&words(args)), ExitCode::from(2), "{args:?}");
  }
}

#[test]
fn a_clean_task_exits_0() {
  assert_eq!(run(&words(&["check-reach"])), ExitCode::SUCCESS);
}
