use toolu_runtime::env::Env;

use super::{Reach, redact};

#[test]
fn every_credential_value_is_redacted_and_blank_ones_are_ignored() {
  let env = Env::from_pairs([
    ("GH_TOKEN", "ghp-sentinel"),
    ("JIRA_PAT", "jira-sentinel"),
    ("LINEAR_API_KEY", " "),
  ]);
  assert_eq!(
    redact(&env, "ghp-sentinel and jira-sentinel failed"),
    "[redacted] and [redacted] failed"
  );
  assert_eq!(redact(&env, "a b"), "a b");
}

#[test]
fn live_clients_refuse_a_missing_credential_with_its_name() {
  let reach = Reach::live();
  let env = Env::from_pairs([("PATH", "/nonexistent")]);
  let err = (reach.tracker)(crate::tracker::TrackerKind::Jira, "PAY-7", "a/b", &env)
    .err()
    .expect("refused");
  assert!(err.contains("JIRA"), "{err}");
}
