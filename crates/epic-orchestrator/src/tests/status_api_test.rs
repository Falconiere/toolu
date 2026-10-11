use std::fs;

use serde_json::json;
use toolu_runtime::config::secrets::Secrets;

use super::{document, route};
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

#[test]
fn status_api_overview_counts_registered_epic_without_excerpt() {
  let temp = tempfile::tempdir().expect("temp root");
  let paths = Paths::at(temp.path());
  let epic = temp.path().join("epic");
  fs::create_dir_all(&epic).expect("epic dir");
  write_value(
    &paths.registry(),
    &json!({
      "version": 1, "epics": [{"key": "repo-402", "state_dir": epic}]
    }),
  )
  .expect("registry");
  write_value(
    &epic.join("graph.json"),
    &json!({
      "epic": "repo-402", "issues": [{
        "key": "repo-449", "repo": "Falconiere/toolu", "title": "Status server",
        "wave": 2, "open_blockers": ["repo-434"], "excerpt": "PRIVATE-EXCERPT-CANARY"
      }]
    }),
  )
  .expect("graph");
  let mut world = World::new(1_000);
  let mut issue = Issue::blank("repo-449", "repo-402", &epic.display().to_string(), 500);
  issue.stage = "running".to_owned();
  issue.phase = "execution".to_owned();
  world.issues.insert(issue.key.clone(), issue);
  let overview = document(&world, &paths, None, &Secrets::none()).expect("overview");
  assert_eq!(overview["schema"], 1);
  assert_eq!(overview["counts"]["running"], 1);
  assert_eq!(overview["epics"][0]["key"], "repo-402");
  assert_eq!(overview["epics"][0]["counts"]["running"], 1);
  assert!(!overview.to_string().contains("PRIVATE-EXCERPT-CANARY"));
  let detail = document(&world, &paths, Some("repo-402"), &Secrets::none()).expect("epic");
  assert_eq!(detail["graph"]["issues"][0]["key"], "repo-449");
  assert_eq!(detail["graph"]["waves"][0]["wave"], 2);
  assert!(!detail.to_string().contains("PRIVATE-EXCERPT-CANARY"));
  let missing =
    document(&world, &paths, Some("missing"), &Secrets::none()).expect_err("unknown selector");
  assert_eq!(missing, "unknown epic or issue: missing");
}

#[test]
fn status_api_static_routes_have_versioned_safe_documents() {
  let (_temp, paths, world) = status_api_fixture();
  for path in [
    "/healthz",
    "/api/v1/overview",
    "/api/v1/epics",
    "/api/v1/epics/repo-402",
    "/api/v1/issues/repo-449",
    "/api/v1/agents",
    "/api/v1/hosts",
    "/api/v1/machine",
    "/api/v1/jobs",
    "/api/v1/queue",
    "/api/v1/attention",
  ] {
    let doc = route(&world, &paths, path, 500, &Secrets::none())
      .expect("route")
      .expect("document");
    assert_eq!(doc["schema"], 1, "{path}");
    assert!(!doc.to_string().contains("PRIVATE-"), "{path}");
  }
  let agents = route(&world, &paths, "/api/v1/agents", 500, &Secrets::none())
    .expect("agents")
    .expect("document");
  assert_eq!(agents["items"][0]["session_id"], "session-a");
  let queue = route(&world, &paths, "/api/v1/queue", 500, &Secrets::none())
    .expect("queue")
    .expect("document");
  assert_eq!(queue["repositories"][0]["repo"], "Falconiere/toolu");
  let worker = route(
    &world,
    &paths,
    "/api/v1/issues/repo-449",
    500,
    &Secrets::none(),
  )
  .expect("worker")
  .expect("document");
  assert_eq!(worker["queue_position"], 1);
}

fn status_api_fixture() -> (tempfile::TempDir, Paths, World) {
  let temp = tempfile::tempdir().expect("temp root");
  let paths = Paths::at(temp.path());
  let epic = temp.path().join("epic");
  fs::create_dir_all(epic.join("issues")).expect("issues dir");
  write_value(
    &paths.registry(),
    &json!({
      "version": 1, "epics": [{"key": "repo-402", "state_dir": epic}]
    }),
  )
  .expect("registry");
  write_value(
    &epic.join("graph.json"),
    &json!({
      "issues": [{"key": "repo-449", "repo": "Falconiere/toolu", "title": "Status",
        "wave": 1, "excerpt": "PRIVATE-EXCERPT", "open_blockers": []}]
    }),
  )
  .expect("graph");
  write_value(
    &epic.join("issues/repo-449.json"),
    &json!({
      "kind": "codex", "worktree": "/tmp/tree", "session_id": "session-a",
      "brief": "PRIVATE-BRIEF"
    }),
  )
  .expect("issue");
  let mut world = World::new(1_000);
  let mut issue = Issue::blank("repo-449", "repo-402", &epic.display().to_string(), 100);
  issue.stage = "running".to_owned();
  issue.phase = "ready".to_owned();
  world.issues.insert(issue.key.clone(), issue);
  (temp, paths, world)
}
