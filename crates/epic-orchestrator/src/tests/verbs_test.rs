use super::{command, run};
use toolu_runtime::cli::Ctx;

#[test]
fn queue_parses_and_nothing_is_planned_any_more() {
  command()
    .try_get_matches_from(["epic", "queue", "one"])
    .expect("queue");
  assert!(command().try_get_matches_from(["epic", "planned"]).is_err());
  command()
    .try_get_matches_from(["epic", "job", "--", "true"])
    .expect("job");
  command()
    .try_get_matches_from(["epic", "wait", "--max-seconds", "0"])
    .expect("wait");
}

#[test]
fn the_action_verbs_parse_and_bad_lines_are_usage_errors() {
  for line in [
    vec![
      "epic",
      "graph",
      "o/r#1",
      "--max",
      "2",
      "--save",
      "--dry-run",
    ],
    vec![
      "epic", "route", "--graph", "g.json", "--issue", "a", "--issue", "b", "--no-jev",
    ],
    vec![
      "epic",
      "launch",
      "--graph",
      "g.json",
      "--issue",
      "a",
      "--kind",
      "codex",
      "--replace",
    ],
    vec!["epic", "finish", "/state", "k-1", "--abandon"],
    vec!["epic", "close", "--graph", "g.json", "--dry-run"],
    vec!["epic", "release", "k-1"],
    vec![
      "epic", "gate", "--pr", "o/r#7", "--merge", "--method", "squash",
    ],
    vec!["epic", "jira", "issue", "get", "PAY-12"],
    vec!["epic", "probe", "--check-evidence"],
  ] {
    assert!(command().try_get_matches_from(&line).is_ok(), "{line:?}");
  }
  for line in [
    vec!["epic", "graph"],
    vec!["epic", "gate", "--pr", "o/r#7", "--method", "fast"],
    vec!["epic", "graph", "o/r#1", "--tracker", "trello"],
    vec!["epic", "probe"],
  ] {
    assert!(command().try_get_matches_from(&line).is_err(), "{line:?}");
  }
}

#[test]
fn the_probe_verb_prints_the_committed_evidence_summary() {
  let matches = command()
    .try_get_matches_from(["epic", "probe", "--check-evidence"])
    .expect("probe");
  let outcome = run(&matches, &Ctx::default(), &());
  assert_eq!(outcome.exit, toolu_protocol::exit::Exit::Success);
  assert!(
    outcome
      .stdout
      .is_some_and(|text| text.starts_with("probe-evidence: valid schema=1 contract=1"))
  );
}
