//! `toolu epic probe --check-evidence` and `toolu epic jira issue get` from the
//! real binary.

#[path = "helpers/epic.rs"]
mod epic;
#[path = "helpers/epic_out.rs"]
mod epic_out;

use epic::{Home, err};
use epic_out::out;

#[test]
fn the_committed_evidence_validates_with_every_gap() {
  let home = Home::new().unwrap();
  let output = home.run(&["epic", "probe", "--check-evidence"]).unwrap();
  assert_eq!(output.status.code(), Some(0), "{}", err(&output));
  let text = out(&output);
  assert!(text.starts_with("probe-evidence: valid schema=1 contract=1\noutcomes: "));
  assert!(text.contains("gaps: ") && !text.contains("gaps: none"));
}

#[test]
fn a_record_without_a_host_is_rejected() {
  let home = Home::new().unwrap();
  let committed = include_str!("../../../docs/toolu/evidence/epic-hosts-v1.json");
  let mut value: serde_json::Value = serde_json::from_str(committed).expect("json");
  value["hosts"]
    .as_object_mut()
    .expect("hosts")
    .remove("cursor");
  let file = home.path("evidence.json");
  std::fs::write(&file, value.to_string()).expect("file");
  let output = home
    .run(&[
      "epic",
      "probe",
      "--check-evidence",
      &file.display().to_string(),
    ])
    .unwrap();
  assert_eq!(output.status.code(), Some(1));
  assert_eq!(out(&output), "");
  assert!(err(&output).contains("evidence.hosts: expected keys claude, codex, cursor, opencode"));
}

#[test]
fn jira_refuses_bad_arguments_and_missing_credentials_before_any_request() {
  let home = Home::new().unwrap();
  let bad = home.run(&["epic", "jira", "issue", "get", "nope"]).unwrap();
  assert_eq!(bad.status.code(), Some(64));
  assert!(err(&bad).contains("Usage: toolu epic jira issue get <KEY>"));
  let missing = home
    .run(&["epic", "jira", "issue", "get", "PAY-12"])
    .unwrap();
  assert_eq!(missing.status.code(), Some(1));
  assert!(err(&missing).contains("JIRA_BASE_URL"));
}
