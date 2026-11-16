use std::path::{Path, PathBuf};

use serde_json::json;
use toolu_runtime::env::Env;

use crate::launch::record::write_record;
use crate::metrics::{usage, usage_report};

fn fixtures() -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/epic/tokens")
}

fn env() -> Env {
  let base = fixtures();
  let claude = base.join("claude");
  let codex = base.join("codex");
  let grok = base.join("grok");
  Env::from_pairs([
    ("HOME", "/nonexistent"),
    ("CLAUDE_CONFIG_DIR", claude.to_str().unwrap()),
    ("CODEX_HOME", codex.to_str().unwrap()),
    ("XDG_DATA_HOME", base.to_str().unwrap()),
    ("GROK_HOME", grok.to_str().unwrap()),
  ])
}

#[test]
fn usage_report_preserves_native_counters_and_reports_partial_known_subtotals() {
  let tmp = tempfile::tempdir().unwrap();
  let dir = tmp.path().join("issues");
  std::fs::create_dir_all(&dir).unwrap();
  for key in ["a", "b", "c", "d", "e", "f"] {
    let data = std::fs::read(fixtures().join(format!("state/issues/{key}.json"))).unwrap();
    let mut record = serde_json::from_slice::<serde_json::Value>(&data)
      .unwrap()
      .as_object()
      .unwrap()
      .clone();
    usage::enroll(&mut record, &env()).unwrap();
    write_record(&dir.join(format!("{key}.json")), &record).unwrap();
  }
  let before = std::fs::read(dir.join("b.json")).unwrap();
  let report = usage_report::report(tmp.path(), &env()).unwrap();
  assert_eq!(report["schema"], 1);
  assert_eq!(report["coverage"], "partial");
  assert_ne!(
    report["reasons"].as_array().unwrap(),
    &[] as &[serde_json::Value]
  );
  assert_eq!(report["attempts"].as_array().unwrap().len(), 6);
  assert_eq!(report["sessions"].as_array().unwrap().len(), 4);
  assert_eq!(report["totals"]["input_tokens"], 766_568);
  assert_eq!(report["totals"]["output_tokens"], 20_474);
  assert_eq!(report["totals"]["total_tokens"], 787_042);
  assert_eq!(std::fs::read(dir.join("b.json")).unwrap(), before);
}

#[test]
fn usage_report_rejects_invalid_history_and_write_failure_is_visible() {
  let tmp = tempfile::tempdir().unwrap();
  let dir = tmp.path().join("issues");
  std::fs::create_dir_all(&dir).unwrap();
  let record = json!({"key":"a", "usage_history":{"schema":99,"legacy_gap":false,"attempts":[]}})
    .as_object()
    .unwrap()
    .clone();
  write_record(&dir.join("a.json"), &record).unwrap();
  let err = usage_report::report(tmp.path(), &env()).unwrap_err();
  assert!(err.contains("usage history"), "{err}");
  let file = tmp.path().join("not-a-directory");
  std::fs::write(&file, "keep").unwrap();
  assert!(write_record(&file.join("a.json"), &record).is_err());
  assert_eq!(std::fs::read_to_string(file).unwrap(), "keep");
}

#[test]
fn usage_report_missing_source_and_null_session_do_not_invent_zero_totals() {
  let tmp = tempfile::tempdir().unwrap();
  let dir = tmp.path().join("issues");
  std::fs::create_dir_all(&dir).unwrap();
  let env = Env::from_pairs([("HOME", "/nonexistent")]);
  for (key, host, session) in [
    ("missing", "codex", json!("missing")),
    ("failed", "cursor", json!(null)),
  ] {
    let mut record = json!({"key":key, "kind":host, "worktree":"/w/a", "launches":1, "session_id":session, "stage":"abandoned"}).as_object().unwrap().clone();
    usage::enroll(&mut record, &env).unwrap();
    write_record(&dir.join(format!("{key}.json")), &record).unwrap();
  }
  let report = usage_report::report(tmp.path(), &env).unwrap();
  assert_eq!(report["coverage"], "unavailable");
  assert_ne!(
    report["reasons"].as_array().unwrap(),
    &[] as &[serde_json::Value]
  );
  assert_eq!(report["attempts"].as_array().unwrap().len(), 2);
  assert_eq!(report["sessions"].as_array().unwrap().len(), 0);
  assert!(report["totals"]["input_tokens"].is_null());
  assert!(report["totals"]["output_tokens"].is_null());
  assert!(report["totals"]["total_tokens"].is_null());
}
