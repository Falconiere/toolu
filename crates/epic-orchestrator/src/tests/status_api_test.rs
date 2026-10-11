use std::fs;

use serde_json::json;
use toolu_runtime::config::secrets::Secrets;

use super::document;
use crate::disk::write_value;
use crate::model::{Issue, World};
use crate::paths::Paths;

#[test]
fn status_api_issue_projects_worker_fields_without_brief_body() {
  let temp = tempfile::tempdir().expect("temp root");
  let paths = Paths::at(temp.path());
  let epic = temp.path().join("epic");
  fs::create_dir_all(epic.join("issues")).expect("issues dir");
  fs::create_dir_all(epic.join("status")).expect("status dir");
  fs::create_dir_all(epic.join("routes")).expect("routes dir");
  let key = "repo-449";
  write_value(
    &epic.join("issues/repo-449.json"),
    &json!({
      "key": key, "kind": "codex", "model": "gpt-6-sol", "effort": "high",
      "worktree": "/tmp/worktree", "session_id": "session-a", "launches": 2,
      "brief": "PRIVATE-BRIEF-CANARY"
    }),
  )
  .expect("issue record");
  write_value(&epic.join("status/repo-449.json"), &json!({
    "phase": "execution", "pr": 123, "history": [{"phase": "spec", "at": "2026-10-11T00:00:00Z", "note": "ok"}]
  })).expect("status record");
  let mut world = World::new(1_000);
  let mut issue = Issue::blank(key, "repo-402", &epic.display().to_string(), 500);
  issue.stage = "running".to_owned();
  issue.phase = "execution".to_owned();
  issue.pr = Some(123);
  world.issues.insert(key.to_owned(), issue);
  let doc = document(&world, &paths, Some(key), &Secrets::none()).expect("status document");
  assert_eq!(doc["schema"], 1);
  assert_eq!(doc["phase_age_ms"], 500);
  assert_eq!(doc["host"], "codex");
  assert_eq!(doc["model"], "gpt-6-sol");
  assert_eq!(doc["pr"], 123);
  assert!(!doc.to_string().contains("PRIVATE-BRIEF-CANARY"));
}
