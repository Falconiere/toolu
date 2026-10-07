use std::path::PathBuf;

use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Ctx;

use super::{command, options, run};

fn matches(words: &[&str]) -> clap::ArgMatches {
  command().try_get_matches_from(words).unwrap()
}

#[test]
fn the_tree_has_every_ledger_verb_with_the_typescript_arguments() {
  let names: Vec<String> = command()
    .get_subcommands()
    .map(|verb| verb.get_name().to_owned())
    .collect();
  assert_eq!(
    names,
    [
      "run",
      "status",
      "preflight",
      "path",
      "root",
      "self-test",
      "verdict"
    ]
  );
  let parsed = matches(&[
    "ledger",
    "run",
    "plan.md",
    "--step",
    "a",
    "--step",
    "b",
    "--activity",
    "x",
    "--force",
    "--verify",
  ]);
  let (_, run_args) = parsed.subcommand().unwrap();
  assert_eq!(
    run_args.get_one::<String>("step").map(String::as_str),
    Some("b"),
    "the last --step wins"
  );
  assert!(run_args.get_flag("force") && run_args.get_flag("verify"));
}

#[test]
fn malformed_command_lines_are_usage_errors() {
  assert!(
    command().try_get_matches_from(["ledger", "run"]).is_err(),
    "run needs a plan doc"
  );
  assert!(
    command()
      .try_get_matches_from(["ledger", "verdict", "table"])
      .is_err()
  );
  assert!(
    command()
      .try_get_matches_from(["ledger", "frobnicate"])
      .is_err()
  );
  assert_eq!(
    matches(&["ledger", "--self-test"]).subcommand_name(),
    Some("self-test")
  );
}

#[test]
fn self_test_answers_in_text_and_json() {
  let text = run(&matches(&["ledger", "self-test"]), &Ctx::default());
  assert_eq!(
    (text.exit, text.stdout.as_deref()),
    (Exit::Success, Some("plan-ledger --self-test: ok"))
  );
  let json = Ctx {
    json: true,
    ..Ctx::default()
  };
  let doc = run(&matches(&["ledger", "--self-test"]), &json);
  assert_eq!(doc.stdout.as_deref(), Some(r#"{"ok":true}"#));
}

#[test]
fn the_config_dir_flag_becomes_the_config_root() {
  let ctx = Ctx {
    config_dir: Some(PathBuf::from("/srv/toolu")),
    ..Ctx::default()
  };
  let opts = options(&ctx).unwrap();
  assert_eq!(opts.env().get("TOOLU_CONFIG_DIR"), Some("/srv/toolu"));
  let missing = clap::ArgMatches::default();
  assert_eq!(run(&missing, &Ctx::default()).exit, Exit::Usage);
}
