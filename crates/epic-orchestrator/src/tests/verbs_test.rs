use super::{command, run};
use crate::paths::Paths;
use crate::query::install_unit;
use toolu_runtime::cli::Ctx;

#[test]
fn planned_lists_only_the_later_verbs() {
  let matches = command()
    .try_get_matches_from(["epic", "planned"])
    .expect("planned");
  let outcome = run(&matches, &Ctx::default(), &());
  assert_eq!(
    outcome.stdout.as_deref(),
    Some("toolu epic is not ported yet (#448). Planned verbs: queue")
  );
  command()
    .try_get_matches_from(["epic", "job", "--", "true"])
    .expect("job");
  command()
    .try_get_matches_from(["epic", "wait", "--max-seconds", "0"])
    .expect("wait");
}

#[test]
fn service_install_writes_a_unit_and_does_not_start_it() {
  let tmp = tempfile::tempdir().expect("temp");
  let home = tmp.path().join("home");
  let paths = Paths::at(tmp.path().join("resources"));
  install_unit(&paths, &home, std::path::Path::new("/usr/bin/toolu")).expect("unit");
  let body = std::fs::read_to_string(paths.service()).expect("service");
  assert!(body.contains("ExecStart=/usr/bin/toolu epic engine"));
  assert!(body.contains("WantedBy=default.target"));
  let user =
    std::fs::read_to_string(home.join(".config/systemd/user/toolu-epic.service")).expect("user");
  assert_eq!(user, body);
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
