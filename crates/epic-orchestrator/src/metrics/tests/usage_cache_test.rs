use std::path::Path;

use serde_json::{Value, json};
use toolu_runtime::env::Env;

use crate::launch::record::write_record;
use crate::metrics::{usage, usage_report};

fn prepare_fixture(home: &Path, missing: &str) {
  let project = home.join("projects/-w-a");
  std::fs::create_dir_all(&project).unwrap();
  let source = Path::new(env!("CARGO_MANIFEST_DIR")).join(
    "../../fixtures/epic/tokens/claude/projects/-w-a/11111111-1111-4111-8111-111111111111.jsonl",
  );
  let source = std::fs::read_to_string(source).unwrap();
  let lines: Vec<String> = source
    .lines()
    .filter_map(|line| {
      let mut entry: Value = serde_json::from_str(line).ok()?;
      if let Some(usage) = entry
        .pointer_mut("/message/usage")
        .and_then(Value::as_object_mut)
      {
        usage.remove(missing);
      }
      Some(entry.to_string())
    })
    .collect();
  std::fs::write(
    project.join("11111111-1111-4111-8111-111111111111.jsonl"),
    lines.join("\n") + "\n",
  )
  .unwrap();
}

#[test]
fn usage_claude_missing_cache_counter_cannot_finalize_an_invented_total() {
  for missing in ["cache_read_input_tokens", "cache_creation_input_tokens"] {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("claude");
    prepare_fixture(&home, missing);
    let env = Env::from_pairs([
      ("HOME", "/nonexistent"),
      ("CLAUDE_CONFIG_DIR", home.to_str().unwrap()),
    ]);
    let mut record = json!({"key":"a", "kind":"claude", "worktree":"/w/a", "launches":0})
      .as_object()
      .unwrap()
      .clone();
    usage::enroll(&mut record, &env).unwrap();
    usage::begin(&mut record, "claude", "2026-10-10T00:00:00Z").unwrap();
    record.insert(
      "session_id".into(),
      json!("11111111-1111-4111-8111-111111111111"),
    );
    usage::attach(&mut record, &env).unwrap();
    usage::checkpoint(&mut record, &env, true).unwrap();
    let issues = tmp.path().join("issues");
    std::fs::create_dir_all(&issues).unwrap();
    write_record(&issues.join("a.json"), &record).unwrap();
    let report = usage_report::report(tmp.path(), &env).unwrap();
    assert_ne!(report["coverage"], "complete", "{missing}");
    assert_eq!(report["attempts"][0]["final"], false, "{missing}");
    assert!(report["totals"]["input_tokens"].is_null(), "{missing}");
    assert!(report["totals"]["total_tokens"].is_null(), "{missing}");
    assert_ne!(
      report["reasons"].as_array().unwrap(),
      &[] as &[Value],
      "{missing}"
    );
  }
}
