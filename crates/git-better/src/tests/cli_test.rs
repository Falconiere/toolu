//! The `check` verb's findings and stdin detection; the binary itself is
//! exercised in `crates/cli/tests/git_better_cli.rs`.

use clap::ArgMatches;

use super::{command, findings, needs_stdin};
use crate::report::Target;
use crate::settings::Settings;

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
fn findings_cover_the_message_and_the_branch() {
  let settings = Settings::default();
  let found = findings(Some("fix stuff"), Some("my-branch"), &settings, false);
  let targets: Vec<Target> = found.iter().map(|finding| finding.target).collect();
  assert_eq!(targets, [Target::Message, Target::Branch]);
  assert_eq!(found[1].name, "my-branch");
  let clean = findings(
    Some("fix(x): y\n\nIntent: z"),
    Some("fix/y"),
    &settings,
    false,
  );
  assert_eq!(clean, []);
  assert_eq!(findings(Some("wip"), None, &settings, true).len(), 1);
}
