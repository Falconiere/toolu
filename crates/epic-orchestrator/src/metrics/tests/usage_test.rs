use std::path::Path;

use serde_json::json;
use toolu_runtime::env::Env;

use crate::launch::record::{load_launch_record, write_record};
use crate::metrics::usage;

fn fixture() -> std::path::PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/epic/tokens")
}

fn env(home: &Path) -> Env {
  Env::from_pairs([
    ("HOME", "/nonexistent"),
    ("CODEX_HOME", home.to_str().unwrap()),
  ])
}

fn source(home: &Path, session: &str, latest: bool) -> std::path::PathBuf {
  let dir = home.join("sessions/2026/10/10");
  std::fs::create_dir_all(&dir).unwrap();
  let file = dir.join(format!("rollout-{session}.jsonl"));
  let original = std::fs::read_to_string(fixture().join("codex/sessions/2026/10/10/rollout-2026-10-10T00-00-00-22222222-2222-4222-8222-222222222222.jsonl")).unwrap();
  let text = if latest {
    original
  } else {
    original.lines().take(3).collect::<Vec<_>>().join("\n")
  };
  std::fs::write(&file, text).unwrap();
  file
}

#[test]
fn usage_history_retains_distinct_sessions_and_terminal_snapshot_after_deletion() {
  let tmp = tempfile::tempdir().unwrap();
  let home = tmp.path().join("codex");
  let env = env(&home);
  let first = "22222222-2222-4222-8222-222222222222";
  let second = "33333333-3333-4333-8333-333333333333";
  let first_source = source(&home, first, false);
  source(&home, second, true);
  let mut record = json!({"key":"b", "kind":"codex", "worktree":"/w/b", "launches":0})
    .as_object()
    .unwrap()
    .clone();
  usage::enroll(&mut record, &env).unwrap();
  usage::begin(&mut record, "codex", "2026-10-10T00:00:00Z").unwrap();
  record.insert("session_id".into(), json!(first));
  usage::attach(&mut record, &env).unwrap();
  usage::checkpoint(&mut record, &env, true).unwrap();
  std::fs::remove_file(first_source).unwrap();
  usage::begin(&mut record, "codex", "2026-10-10T01:00:00Z").unwrap();
  record.insert("session_id".into(), json!(second));
  usage::attach(&mut record, &env).unwrap();
  usage::checkpoint(&mut record, &env, true).unwrap();
  usage::checkpoint(&mut record, &env, true).unwrap();
  std::fs::create_dir_all(tmp.path().join("issues")).unwrap();
  let path = tmp.path().join("issues/b.json");
  write_record(&path, &record).unwrap();
  let loaded = load_launch_record(&path).unwrap();
  let history = &loaded["usage_history"];
  assert_eq!(history["schema"], 1);
  let attempts = history["attempts"].as_array().unwrap();
  assert_eq!(attempts.len(), 2);
  assert_eq!(attempts[0]["session"], first);
  assert_eq!(attempts[0]["final"], true);
  assert_eq!(attempts[0]["row"]["tokens"]["input_tokens"], 100);
  assert_eq!(attempts[1]["session"], second);
  assert_eq!(attempts[1]["row"]["tokens"]["input_tokens"], 500);
  let report = crate::metrics::usage_report::report(tmp.path(), &env).unwrap();
  assert_eq!(report["sessions"].as_array().unwrap().len(), 2);
  assert_eq!(report["totals"]["input_tokens"], 600);
  assert_eq!(report["totals"]["output_tokens"], 60);
}

#[test]
fn usage_history_resumed_session_counts_latest_cumulative_snapshot_once() {
  let tmp = tempfile::tempdir().unwrap();
  let home = tmp.path().join("codex");
  let env = env(&home);
  let session = "22222222-2222-4222-8222-222222222222";
  source(&home, session, false);
  let mut record = json!({"key":"b", "kind":"codex", "worktree":"/w/b", "launches":0})
    .as_object()
    .unwrap()
    .clone();
  usage::enroll(&mut record, &env).unwrap();
  usage::begin(&mut record, "codex", "2026-10-10T00:00:00Z").unwrap();
  record.insert("session_id".into(), json!(session));
  usage::attach(&mut record, &env).unwrap();
  usage::checkpoint(&mut record, &env, true).unwrap();
  source(&home, session, true);
  usage::begin(&mut record, "codex", "2026-10-10T01:00:00Z").unwrap();
  usage::attach(&mut record, &env).unwrap();
  usage::checkpoint(&mut record, &env, true).unwrap();
  std::fs::create_dir_all(tmp.path().join("issues")).unwrap();
  write_record(&tmp.path().join("issues/b.json"), &record).unwrap();
  let report = crate::metrics::usage_report::report(tmp.path(), &env).unwrap();
  assert_eq!(report["attempts"].as_array().unwrap().len(), 2);
  assert_eq!(report["sessions"].as_array().unwrap().len(), 1);
  assert_eq!(report["totals"]["input_tokens"], 500);
  assert_eq!(report["totals"]["output_tokens"], 50);
  assert_eq!(report["totals"]["total_tokens"], 550);
}

#[test]
fn usage_history_rejects_unknown_or_duplicate_history_before_checkpoint() {
  let env = env(Path::new("/nonexistent"));
  for history in [
    json!({"schema":2,"legacy_gap":false,"attempts":[]}),
    json!({"schema":1,"legacy_gap":false,"attempts":[{"id":1,"host":"codex","session":null,"worktree":"/w/a","started_at":"2026-10-10T00:00:00Z","observed_at":null,"final":false,"row":{"status":"unavailable","reason":"no session"}},{"id":1,"host":"codex","session":null,"worktree":"/w/a","started_at":"2026-10-10T00:00:00Z","observed_at":null,"final":false,"row":{"status":"unavailable","reason":"no session"}}]}),
  ] {
    let mut record = json!({"usage_history":history})
      .as_object()
      .unwrap()
      .clone();
    let err = usage::enroll(&mut record, &env).unwrap_err();
    assert!(err.contains("usage history"), "{err}");
  }
}
