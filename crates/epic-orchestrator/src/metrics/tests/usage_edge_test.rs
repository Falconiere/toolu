use std::path::Path;

use serde_json::json;
use toolu_runtime::env::Env;

use crate::launch::record::{load_launch_record, write_record};
use crate::metrics::{usage, usage_report};

#[test]
fn usage_edge_stale_live_snapshot_retains_observation_and_terminal_snapshot_survives() {
  let tmp = tempfile::tempdir().unwrap();
  let home = tmp.path().join("codex");
  let source = home.join("sessions/rollout-22222222-2222-4222-8222-222222222222.jsonl");
  std::fs::create_dir_all(source.parent().unwrap()).unwrap();
  let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/epic/tokens/codex/sessions/2026/10/10/rollout-2026-10-10T00-00-00-22222222-2222-4222-8222-222222222222.jsonl");
  std::fs::copy(&fixture, &source).unwrap();
  let env = Env::from_pairs([
    ("HOME", "/nonexistent"),
    ("CODEX_HOME", home.to_str().unwrap()),
  ]);
  let mut record = json!({"key":"b", "kind":"codex", "worktree":"/w/b", "launches":0})
    .as_object()
    .unwrap()
    .clone();
  usage::enroll(&mut record, &env).unwrap();
  usage::begin(&mut record, "codex", "2026-10-10T00:00:00Z").unwrap();
  record.insert(
    "session_id".into(),
    json!("22222222-2222-4222-8222-222222222222"),
  );
  usage::attach(&mut record, &env).unwrap();
  let observed = record["usage_history"]["attempts"][0]["observed_at"].clone();
  assert!(observed.as_str().is_some_and(|s| !s.is_empty()));
  let issues = tmp.path().join("issues");
  std::fs::create_dir_all(&issues).unwrap();
  let path = issues.join("b.json");
  write_record(&path, &record).unwrap();
  std::fs::remove_file(&source).unwrap();
  let stale = usage_report::report(tmp.path(), &env).unwrap();
  assert_stale(&stale, &observed);
  assert_ne!(stale["attempts"][0]["row"]["reason"].as_str().unwrap(), "");
  assert_ne!(
    stale["reasons"].as_array().unwrap(),
    &[] as &[serde_json::Value]
  );
  std::fs::copy(fixture, &source).unwrap();
  usage::checkpoint(&mut record, &env, true).unwrap();
  write_record(&path, &record).unwrap();
  std::fs::remove_file(source).unwrap();
  let terminal = usage_report::report(tmp.path(), &env).unwrap();
  assert_eq!(terminal["coverage"], "complete");
  assert_eq!(terminal["totals"]["total_tokens"], 550);
  assert_eq!(terminal["attempts"][0]["final"], true);
  invalid_history_is_rejected(&path);
}

fn assert_stale(stale: &serde_json::Value, observed: &serde_json::Value) {
  assert_eq!(stale["coverage"], "partial");
  assert_eq!(stale["totals"]["total_tokens"], 550);
  assert_eq!(&stale["attempts"][0]["observed_at"], observed);
  assert_eq!(stale["attempts"][0]["row"]["status"], "stale");
}

fn invalid_history_is_rejected(path: &Path) {
  let invalid =
    json!({"key":"b", "usage_history":{"schema":99,"legacy_gap":false,"attempts":[]}}).to_string();
  std::fs::write(path, &invalid).unwrap();
  let error = load_launch_record(path).unwrap_err();
  assert!(error.contains("usage history"), "{error}");
  assert_eq!(std::fs::read_to_string(path).unwrap(), invalid);
}
