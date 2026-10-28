use super::{command, effort};
use crate::budget::Effort;

#[test]
fn effort_is_unset_by_default_and_takes_every_level() {
  let run = |args: &[&str]| {
    let matches = command().try_get_matches_from(args).unwrap();
    effort(matches.subcommand_matches("run").unwrap())
  };
  assert_eq!(run(&["code-review", "run"]), None);
  for level in Effort::ALL {
    assert_eq!(
      run(&["code-review", "run", "--effort", level.name()]),
      Some(level)
    );
  }
}

#[test]
fn an_unknown_effort_or_a_missing_verb_is_a_usage_error() {
  let err = command()
    .try_get_matches_from(["code-review", "run", "--effort", "huge"])
    .unwrap_err();
  assert_eq!(err.kind(), clap::error::ErrorKind::InvalidValue);
  assert!(command().try_get_matches_from(["code-review"]).is_err());
}

#[test]
fn run_takes_a_base_and_repeated_paths() {
  let matches = command()
    .try_get_matches_from([
      "code-review",
      "run",
      "--base",
      "main",
      "--path",
      "a",
      "--path",
      "b",
    ])
    .unwrap();
  let run = matches.subcommand_matches("run").unwrap();
  assert_eq!(
    run.get_one::<String>("base").map(String::as_str),
    Some("main")
  );
  let paths: Vec<&String> = run.get_many::<String>("path").unwrap().collect();
  assert_eq!(paths, ["a", "b"]);
}
