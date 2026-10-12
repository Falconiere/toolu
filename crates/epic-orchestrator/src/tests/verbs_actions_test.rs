use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Ctx;
use toolu_runtime::env::Env;

use super::{commands, run};

#[test]
fn every_action_verb_is_listed_and_others_fall_through() {
  let names: Vec<String> = commands()
    .iter()
    .map(|command| command.get_name().to_owned())
    .collect();
  assert_eq!(
    names,
    [
      "graph", "route", "launch", "finish", "close", "release", "gate", "jira", "probe"
    ]
  );
  let matches = crate::verbs::command()
    .try_get_matches_from(["epic", "queue"])
    .expect("queue");
  let env = Env::from_pairs([("HOME", "/tmp")]);
  assert!(run("queue", &matches, &Ctx::default(), &env).is_none());
}

#[test]
fn a_bad_reference_is_a_usage_error() {
  let matches = crate::verbs::command()
    .try_get_matches_from(["epic", "gate", "--pr", "nonsense"])
    .expect("gate");
  let gate = matches.subcommand_matches("gate").expect("gate");
  let outcome = run(
    "gate",
    gate,
    &Ctx::default(),
    &Env::from_pairs([("HOME", "/tmp")]),
  )
  .expect("verb");
  assert_eq!(outcome.exit, Exit::Usage);
  assert!(
    outcome
      .stderr
      .is_some_and(|text| text.starts_with("toolu epic gate: unrecognized issue reference"))
  );
}
