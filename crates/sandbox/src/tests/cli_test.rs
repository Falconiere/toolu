use clap::error::ErrorKind;

use super::{argv, command, set};

fn verb(args: &[&str]) -> clap::ArgMatches {
  let matches = command().try_get_matches_from(args).unwrap();
  matches.subcommand().unwrap().1.clone()
}

#[test]
fn every_verb_parses() {
  for name in ["setup", "status", "start", "stop", "shell"] {
    let matches = command().try_get_matches_from(["sandbox", name]).unwrap();
    assert_eq!(matches.subcommand_name(), Some(name));
  }
  assert!(set(&verb(&["sandbox", "start", "--restart"]), "restart"));
  assert!(!set(&verb(&["sandbox", "start"]), "restart"));
  assert!(set(&verb(&["sandbox", "delete", "--yes"]), "yes"));
}

#[test]
fn run_keeps_the_whole_command_including_its_flags() {
  let matches = verb(&[
    "sandbox", "run", "--clone", "--", "cargo", "test", "-p", "x",
  ]);
  assert!(set(&matches, "clone"));
  assert_eq!(argv(&matches), ["cargo", "test", "-p", "x"]);
  let bare = verb(&["sandbox", "run", "ls", "-la", "--clone"]);
  assert!(!set(&bare, "clone"));
  assert_eq!(argv(&bare), ["ls", "-la", "--clone"]);
  assert!(!set(&bare, "no-such-flag"));
}

#[test]
fn a_missing_verb_command_or_confirmation_is_a_usage_error() {
  assert!(command().try_get_matches_from(["sandbox"]).is_err());
  let run = command()
    .try_get_matches_from(["sandbox", "run"])
    .unwrap_err();
  assert_eq!(run.kind(), ErrorKind::MissingRequiredArgument);
  let delete = command()
    .try_get_matches_from(["sandbox", "delete"])
    .unwrap_err();
  assert_eq!(delete.kind(), ErrorKind::MissingRequiredArgument);
  assert_eq!(argv(&verb(&["sandbox", "status"])), Vec::<String>::new());
}
