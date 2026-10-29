use super::verify;
use crate::launch::record::Record;
use serde_json::json;

#[test]
fn rejects_foreign_runs_bases_and_noncommit_results() {
  let sha = "a".repeat(40);
  let mut record = Record::new();
  record.insert("hc_run".into(), json!("run-1"));
  record.insert("expected_base".into(), json!(sha));
  let report = json!({"run":"run-1","expected_base":sha,"commit":"b".repeat(40),"phase":"ready"});
  assert!(verify(&record, &report).is_ok());
  for field in ["run", "expected_base", "commit", "phase"] {
    let mut invalid = report.clone();
    invalid[field] = json!("wrong");
    assert!(verify(&record, &invalid).is_err());
  }
}

#[test]
fn a_completed_result_cannot_publish_into_a_replacement_run() {
  let root = tempfile::tempdir().unwrap();
  let state = root.path().join("epic");
  std::fs::create_dir_all(state.join("issues")).unwrap();
  let sha = "a".repeat(40);
  let record = json!({"stage":"running","hc_run":"new-run","expected_base":sha,
    "agent":"hc:t2"});
  crate::disk::write_value(&state.join("issues/a.json"), &record).unwrap();
  let mut engine = crate::server::Engine::open(
    crate::paths::Paths::at(root.path()),
    None,
    crate::server::Fault::None,
  )
  .unwrap();
  let mut issue = crate::model::Issue::blank("a", "epic", state.to_str().unwrap(), 0);
  issue.agent = "hc:t2".into();
  engine.world.issues.insert("a".into(), issue.clone());
  let report = json!({"run":"old-run","expected_base":sha,"commit":"b".repeat(40),"phase":"ready"});
  let error = super::complete_locked(&mut engine, "a", &issue, &report).unwrap_err();
  assert!(error.contains("run or base differs"));
  assert_eq!(engine.world.issues["a"].agent, "hc:t2");
  assert!(engine.world.outbox.is_empty());
}
