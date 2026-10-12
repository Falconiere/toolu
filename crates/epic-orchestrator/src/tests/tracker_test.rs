use toolu_runtime::env::Env;

use super::{TrackerKind, make_tracker};

#[test]
fn make_tracker_refuses_bad_references_and_missing_settings() {
  let env = Env::from_pairs([("HOME", "/nonexistent"), ("GH_TOKEN", "t")]);
  assert!(make_tracker(TrackerKind::Github, "not a ref", "", &env).is_err());
  let jira = make_tracker(TrackerKind::Jira, "PAY-7", "a/b", &env).err();
  assert!(jira.is_some_and(|err| err.starts_with("Jira requires JIRA_BASE_URL")));
  let linear = make_tracker(TrackerKind::Linear, "ENG-1", "a/b", &env).err();
  assert_eq!(linear.as_deref(), Some("LINEAR_API_KEY is not set"));
}

#[test]
fn make_tracker_builds_each_tracker_from_settings() {
  let env = Env::from_pairs([
    ("HOME", "/nonexistent"),
    ("GH_TOKEN", "t"),
    ("JIRA_BASE_URL", "https://acme.atlassian.net"),
    ("JIRA_PAT", "p"),
    ("LINEAR_API_KEY", "k"),
  ]);
  let github = make_tracker(TrackerKind::Github, "o/r#1", "", &env).expect("github");
  assert_eq!(
    (github.kind(), github.epic_ref()),
    (TrackerKind::Github, "o/r#1")
  );
  let jira = make_tracker(TrackerKind::Jira, "jira:pay-7", "a/b", &env).expect("jira");
  assert_eq!(jira.epic_ref(), "PAY-7");
  let linear = make_tracker(TrackerKind::Linear, "ENG-1", "a/b", &env).expect("linear");
  assert_eq!(linear.kind(), TrackerKind::Linear);
}
