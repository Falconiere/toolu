//! The namespace tree and which verbs read stdin.

use clap::ArgMatches;

use super::{command, needs_stdin};

fn matches(args: &[&str]) -> ArgMatches {
  command().try_get_matches_from(args).unwrap()
}

#[test]
fn only_a_message_file_of_dash_reads_stdin() {
  assert!(needs_stdin(&matches(&["git-better", "check", "-F", "-"])));
  assert!(!needs_stdin(&matches(&[
    "git-better",
    "check",
    "-F",
    "msg.txt"
  ])));
  assert!(!needs_stdin(&matches(&["git-better", "check", "-m", "x"])));
  assert!(
    command()
      .try_get_matches_from(["git-better", "check", "-m", "x", "-F", "y"])
      .is_err()
  );
}

#[test]
fn the_namespace_has_check_and_setup() {
  let tree = command();
  let names: Vec<&str> = tree
    .get_subcommands()
    .map(clap::Command::get_name)
    .collect();
  assert_eq!(names, ["check", "setup"]);
}
