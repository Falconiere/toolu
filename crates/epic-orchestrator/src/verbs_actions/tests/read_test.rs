use toolu_protocol::exit::Exit;
use toolu_runtime::env::Env;

use super::{jira, probe};
use crate::support::matches;

#[test]
fn jira_refuses_bad_keys_as_usage_and_missing_credentials_as_failure() {
  let env = Env::from_pairs([("HOME", "/tmp")]);
  let bad = jira(&matches(&["epic", "jira", "issue", "get", "nope"]), &env);
  assert_eq!(
    (bad.exit, bad.stderr.as_deref()),
    (
      Exit::Usage,
      Some("jira issue: Usage: toolu epic jira issue get <KEY>")
    )
  );
  let missing = jira(&matches(&["epic", "jira", "issue", "get", "PAY-12"]), &env);
  assert_eq!(missing.exit, Exit::Failure);
}

#[test]
fn probe_checks_a_given_file_and_rejects_a_missing_host() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut value: serde_json::Value =
    serde_json::from_str(crate::probe_evidence::COMMITTED).expect("json");
  value["hosts"]
    .as_object_mut()
    .expect("hosts")
    .remove("codex");
  let file = tmp.path().join("evidence.json");
  std::fs::write(&file, value.to_string()).expect("file");
  let file = file.display().to_string();
  let outcome = probe(&matches(&["epic", "probe", "--check-evidence", &file]));
  assert_eq!(outcome.exit, Exit::Failure);
  assert!(outcome.stderr.is_some_and(|text| {
    text.starts_with("probe-evidence: invalid: evidence.hosts: expected keys")
  }));
  let missing = probe(&matches(&[
    "epic",
    "probe",
    "--check-evidence",
    "/nonexistent.json",
  ]));
  assert_eq!(missing.exit, Exit::Failure);
}
