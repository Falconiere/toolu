use serde_json::{Value, json};

use super::{COMMITTED, check_evidence, summarize, validate};

#[test]
fn the_committed_evidence_keeps_every_capability_gap() {
  let value: Value = serde_json::from_str(COMMITTED).expect("json");
  let hosts = validate(&value).expect("valid");
  assert_eq!(
    hosts.keys().map(String::as_str).collect::<Vec<_>>(),
    ["claude", "codex", "cursor", "opencode"]
  );
  assert!(hosts.values().all(|caps| caps.len() == 6));
  let summary = summarize(&hosts);
  assert!(summary.starts_with("probe-evidence: valid schema=1 contract=1\noutcomes: verified="));
  assert!(
    summary.contains("cursor.start=unverified") && summary.contains("claude.teardown=unverified")
  );
  assert!(!summary.contains("codex.cancel_replace=failed") && !summary.contains("gaps: none"));
  assert_eq!(check_evidence(None), Ok(summary));
}

#[test]
fn a_record_without_a_host_is_rejected_with_the_expected_keys() {
  let mut value: Value = serde_json::from_str(COMMITTED).expect("json");
  value["hosts"]
    .as_object_mut()
    .expect("hosts")
    .remove("cursor");
  let err = check_evidence(Some(&value.to_string())).expect_err("missing host");
  assert_eq!(
    err,
    "probe-evidence: invalid: evidence.hosts: expected keys claude, codex, cursor, opencode; got claude, codex, opencode"
  );
}

#[test]
fn bad_outcomes_stamps_revisions_and_failures_name_their_path() {
  let base: Value = serde_json::from_str(COMMITTED).expect("json");
  let cases = [
    (
      "/hosts/codex/capabilities/start/outcome",
      json!("great"),
      "evidence.hosts.codex.capabilities.start.outcome: expected one of verified, failed, unsupported, unverified",
    ),
    (
      "/hosts/codex/capabilities/start/observedAt",
      json!("yesterday"),
      "evidence.hosts.codex.capabilities.start.observedAt: expected UTC ISO-8601 timestamp",
    ),
    (
      "/source/revision",
      json!("abc"),
      "evidence.source.revision: expected full git SHA",
    ),
    (
      "/schemaVersion",
      json!(2),
      "evidence.schemaVersion: expected 1",
    ),
  ];
  for (pointer, bad, message) in cases {
    let mut value = base.clone();
    *value.pointer_mut(pointer).expect("pointer") = bad;
    assert_eq!(validate(&value).expect_err(pointer), message);
  }
  let mut failed = base;
  let start = failed
    .pointer_mut("/hosts/claude/capabilities/start")
    .expect("start");
  start["outcome"] = json!("failed");
  start["nativeError"] = Value::Null;
  assert_eq!(
    validate(&failed).expect_err("failed"),
    "evidence.hosts.claude.capabilities.start.nativeError: failed capability requires the native failure"
  );
  assert!(
    check_evidence(Some("{"))
      .expect_err("json")
      .starts_with("probe-evidence: invalid: ")
  );
}
