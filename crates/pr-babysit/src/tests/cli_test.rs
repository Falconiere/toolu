use clap::error::ErrorKind;

use super::command;

fn parse(args: &[&str]) -> Result<clap::ArgMatches, clap::Error> {
  command().try_get_matches_from(std::iter::once("babysit").chain(args.iter().copied()))
}

#[test]
fn the_tree_is_consistent_and_lists_every_verb() {
  command().debug_assert();
  let verbs: Vec<String> = command()
    .get_subcommands()
    .map(|verb| verb.get_name().to_owned())
    .collect();
  let expected = [
    "tick",
    "wait",
    "collect",
    "reduce-state",
    "parse-verdict",
    "record",
    "reply",
    "resolve",
    "route-fix",
    "dispatch-fix",
    "fixer-report",
    "stop",
  ];
  assert_eq!(verbs, expected);
}

#[test]
fn a_pull_request_read_needs_a_positive_pr_and_defaults_its_page_size() {
  let matches = parse(&[
    "tick",
    "--repo",
    "o/r",
    "--pr",
    "7",
    "--state-file",
    "s.json",
  ])
  .unwrap();
  let (_, tick) = matches.subcommand().unwrap();
  assert_eq!(tick.get_one::<u64>("page-size"), Some(&100));
  let zero = parse(&[
    "tick",
    "--repo",
    "o/r",
    "--pr",
    "0",
    "--state-file",
    "s.json",
  ]);
  assert_eq!(zero.unwrap_err().kind(), ErrorKind::ValueValidation);
  let missing = parse(&["wait", "--repo", "o/r", "--state-file", "s.json"]);
  assert_eq!(
    missing.unwrap_err().kind(),
    ErrorKind::MissingRequiredArgument
  );
}

#[test]
fn record_round_accepts_only_true_or_false() {
  let round = [
    "record",
    "round",
    "--state-file",
    "s.json",
    "--had-rejection",
  ];
  assert!(parse(&[&round[..], &["true", "--fix-pushed"]].concat()).is_ok());
  let maybe = parse(&[&round[..], &["maybe"]].concat());
  assert_eq!(maybe.unwrap_err().kind(), ErrorKind::InvalidValue);
}

#[test]
fn fixer_report_takes_the_report_and_a_closed_status() {
  assert!(parse(&["fixer-report", "r.json", "done", "--note", "fixed"]).is_ok());
  let unknown = parse(&["fixer-report", "r.json", "maybe"]);
  assert_eq!(unknown.unwrap_err().kind(), ErrorKind::InvalidValue);
}

#[test]
fn route_fix_collects_every_raise() {
  let matches = parse(&[
    "route-fix",
    "--items",
    "i.json",
    "--raise",
    "a",
    "--raise",
    "b",
  ])
  .unwrap();
  let (_, route) = matches.subcommand().unwrap();
  let raised: Vec<&String> = route.get_many::<String>("raise").unwrap().collect();
  assert_eq!(raised, ["a", "b"]);
}

#[test]
fn remote_target_and_report_identity_are_paired() {
  let start = [
    "dispatch-fix",
    "start",
    "--state-file",
    "s",
    "--plan",
    "p",
    "--items",
    "i",
    "--repo-root",
    "/r",
    "--branch",
    "b",
    "--base",
    "main",
  ];
  assert!(parse(&[&start[..], &["--machine", "vm"]].concat()).is_err());
  assert!(parse(&[&start[..], &["--machine", "vm", "--remote-repo", "/r"]].concat()).is_ok());
  assert!(parse(&["fixer-report", "r", "done", "--run-id", "run"]).is_err());
}
