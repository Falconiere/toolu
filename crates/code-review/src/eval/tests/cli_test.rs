use super::{command, settings_of};
use crate::eval::Setting;

fn parse(args: &[&str]) -> Result<clap::ArgMatches, clap::Error> {
  command().try_get_matches_from(std::iter::once("eval").chain(args.iter().copied()))
}

#[test]
fn only_cases_is_required_and_the_default_is_both_arms_without_live() {
  assert!(parse(&[]).is_err());
  let matches = parse(&["--cases", "set"]).expect("parses");
  assert_eq!(
    matches.get_one::<String>("arm").map(String::as_str),
    Some("both")
  );
  assert!(!matches.get_flag("live"));
  assert_eq!(
    matches.get_many::<String>("effort").map(Iterator::count),
    None
  );
  assert_eq!(matches.get_one::<String>("replay-dir"), None);
  assert_eq!(matches.get_one::<String>("report"), None);
}

#[test]
fn effort_repeats_and_accepts_none_but_nothing_else_unknown() {
  let matches = parse(&[
    "--cases", "set", "--effort", "auto", "--effort", "none", "--effort", "xhigh",
  ])
  .expect("parses");
  let levels: Vec<&str> = matches
    .get_many::<String>("effort")
    .expect("given")
    .map(String::as_str)
    .collect();
  assert_eq!(levels, ["auto", "none", "xhigh"]);
  assert!(parse(&["--cases", "set", "--effort", "huge"]).is_err());
  assert!(parse(&["--cases", "set", "--arm", "third"]).is_err());
}

#[test]
fn live_and_the_paths_are_read_as_given() {
  let matches = parse(&[
    "--cases",
    "set",
    "--live",
    "--replay-dir",
    "blobs",
    "--report",
    "out.json",
    "--arm",
    "enhanced",
  ])
  .expect("parses");
  assert!(matches.get_flag("live"));
  assert_eq!(
    matches.get_one::<String>("replay-dir").map(String::as_str),
    Some("blobs")
  );
  assert_eq!(
    matches.get_one::<String>("report").map(String::as_str),
    Some("out.json")
  );
  assert_eq!(
    matches.get_one::<String>("arm").map(String::as_str),
    Some("enhanced")
  );
}

#[test]
fn a_repeated_effort_runs_once_in_first_seen_order() {
  let matches = parse(&[
    "--cases", "set", "--effort", "small", "--effort", "auto", "--effort", "small",
  ])
  .expect("parses");
  assert_eq!(settings_of(&matches), [Setting::Small, Setting::Auto]);
  let default = parse(&["--cases", "set"]).expect("parses");
  assert_eq!(settings_of(&default), [Setting::Auto]);
}
