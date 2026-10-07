use serde_json::json;

use super::{read, review};

const GATE: &str = r#"{
  "status": "failing",
  "reason": "lint",
  "source": "ts-quality",
  "file": "src/b.ts",
  "violations": "b\n",
  "entries": {
    "src/a.ts": {"source": "ts-quality", "reason": "lint", "violations": "a\n", "updatedAt": "t1"},
    "src/b.ts": {"source": "ts-quality", "reason": "lint", "violations": "b\n", "updatedAt": "t2"}
  },
  "updatedAt": "t2"
}"#;

#[test]
fn a_failing_gate_keeps_both_slots_and_a_review_is_recorded() {
  let dir = tempfile::tempdir().unwrap();
  let gate = dir.path().join("quality-gate-status.json");
  std::fs::write(&gate, GATE).unwrap();
  let value = read(Some(&gate));
  assert_eq!(value["status"], "failing");
  assert_eq!(value["entries"].as_array().map(Vec::len), Some(2));
  assert_eq!(value["entries"][0]["file"], "src/a.ts");
  assert_eq!(value["entries"][0]["violations"], "a\n");

  let review_dir = dir.path().join("push-review");
  std::fs::create_dir_all(&review_dir).unwrap();
  std::fs::write(
    review_dir.join("feat_x.json"),
    r#"{"version":2,"diff_sha":"abc","review_round":1,"findings_count":0,"reviewed_files":["src/a.ts"]}"#,
  )
  .unwrap();
  std::fs::write(
    review_dir.join("feat_x.waiver.json"),
    r#"{"diff_sha":"abc"}"#,
  )
  .unwrap();
  let (found, waivers) = review(Some(dir.path()), "feat/x");
  assert_eq!(found["state"], "recorded");
  assert_eq!(found["document"]["review_round"], 1);
  assert_eq!(found["document"]["reviewed_files"], json!(["src/a.ts"]));
  let path = found["path"].as_str().unwrap_or("");
  assert!(path.contains("feat_x.json"), "{path}");
  assert_eq!(waivers["waiver"]["diff_sha"], "abc");
  assert!(waivers["pending"].is_null());

  let (missing, none) = review(Some(dir.path()), "");
  assert_eq!(missing["state"], "missing");
  assert!(none["waiver"].is_null());
  assert_eq!(read(None)["status"], "missing");
}
