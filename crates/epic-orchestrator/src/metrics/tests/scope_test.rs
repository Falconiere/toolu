use super::Scope;
use crate::journal::Record;

fn record(name: &str, epic: &str, key: &str, note: &str) -> Record {
  let mut row = Record::new("action", name, key, "", note);
  row.epic = epic.to_owned();
  row
}

fn learned(epic: Option<&str>, rows: &[Record]) -> Scope {
  let mut scope = Scope::new(epic);
  for row in rows {
    scope.learn(row);
  }
  scope
}

#[test]
fn an_issue_seen_with_its_epic_keeps_its_unstamped_records() {
  let stamped = record("phase", "e1", "i1", "ready");
  let admission = record("job", "", "i1", "cargo test");
  let other = record("phase", "e2", "i2", "ready");
  let scope = learned(
    Some("e1"),
    &[stamped.clone(), admission.clone(), other.clone()],
  );
  assert!(scope.keeps(&stamped));
  assert!(scope.keeps(&admission));
  assert!(!scope.keeps(&other));
  assert!(!scope.keeps(&record("job", "", "unknown", "")));
}

#[test]
fn github_attempts_follow_their_issue_and_name_its_repository() {
  let check = record(
    "github-check",
    "",
    "pr:o/r#7",
    r#"{"issue":"i1","cause":"scheduled"}"#,
  );
  let other = record("github-check", "", "pr:o/r#8", r#"{"issue":"i2"}"#);
  let rows = [
    record("phase", "e1", "i1", "ready"),
    check.clone(),
    other.clone(),
  ];
  let scope = learned(Some("e1"), &rows);
  assert!(scope.keeps(&check));
  assert!(!scope.keeps(&other));
  assert_eq!(scope.issue("pr:o/r#7"), Some("i1"));
  assert_eq!(scope.issue("pr:o/r#9"), None);
  assert_eq!(scope.repo("i1"), Some("o/r"));
}

#[test]
fn keyless_engine_records_count_in_every_scope_but_others_do_not() {
  let scope = learned(Some("e1"), &[]);
  assert!(scope.keeps(&record("wait-reply", "", "", "timeout")));
  assert!(scope.keeps(&record("engine-usage", "", "", "{}")));
  assert!(!scope.keeps(&record("herdr-snapshot", "", "", "")));
  assert!(scope.keeps(&record("epic-closed", "e1", "e1", "")));
  assert!(Scope::new(None).keeps(&record("herdr-snapshot", "", "", "")));
}
